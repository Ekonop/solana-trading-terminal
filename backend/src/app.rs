//! Application lifecycle: configuration -> database boot -> services -> HTTP.

use std::sync::Arc;

use tokio::sync::watch;

use crate::api;
use crate::config::{AppMode, Config};
use crate::db::Database;
use crate::domain::*;
use crate::errors::AppError;
use crate::events::{EventBus, WsEvent};
use crate::market_data::cache::MarketCache;
use crate::market_data::models::ProviderHealth;
use crate::market_data::providers::dexscreener::DexScreenerProvider;
use crate::market_data::providers::mock::MockProvider;
use crate::market_data::providers::MarketProvider;
use crate::market_data::service::{MarketDataService, MarketDataServiceHandle};
use crate::repos;
use crate::risk::RiskManager;
use crate::strategies::service::{StrategyService, StrategyServiceHandle};
use crate::strategies::registry;
use crate::time::now_micros;
use crate::trading::executor::{Executor, ExecutorHandle};
use crate::trading::portfolio;
use crate::version;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shared state handed to the axum router and WS sessions.
#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub db: Database,
    pub bus: EventBus,
    pub cache: Arc<MarketCache>,
    pub exec: ExecutorHandle,
    pub strategies: StrategyServiceHandle,
    pub market_data: MarketDataServiceHandle,
    pub real: Arc<crate::real::RealService>,
    pub rulebook: Arc<crate::agent::RulebookService>,
    pub account_id: String,
    pub sol_mint: String,
    pub started_at: i64,
    pub shutdown: watch::Sender<bool>,
}

impl AppState {
    pub fn version(&self) -> String {
        version::VERSION.to_string()
    }
}

/// Boot the whole application. Returns when shutdown is requested.
pub async fn run(cfg: Config) -> Result<(), AppError> {
    let started_at = now_micros();
    let cfg = Arc::new(cfg);

    tracing::info!(version = %APP_VERSION, mode = %cfg.application.mode, "starting solana trading terminal (paper-only)");

    // 1. Database: exactly one PGlite instance for this process.
    let db = Database::open(&cfg.database.data_directory).await?;
    crate::db::migrations::migrate(&db).await?;
    db.verify().await?;
    tracing::info!(data_dir = %db.data_dir().display(), "embedded postgres ready");

    // 2. Default paper account (demo balance, lamp-vs-decimal accounting).
    let account = repos::accounts::get_or_create_default_account(
        &db,
        cfg.paper_trading.initial_balance_lamports,
    )
    .await?;
    let account_id = account.id.clone();
    tracing::info!(account = %account_id, "paper account ready");

    // 3. Register the five strategies and their persisted configs.
    registry::register_all(&db).await?;
    // Never auto-restart strategies from a previous session: trading state is
    // inconsistent after restart (positions may reference stale runs). Clear.
    let strategies = repos::strategies::list_strategies(&db).await?;
    for s in strategies {
        if s.status == StrategyState::PaperTrading || s.status == StrategyState::Paused {
            repos::strategies::set_strategy_status(&db, &s.id, StrategyState::Ready).await?;
            repos::events::record_system_event(
                &db,
                SystemLevel::Info,
                "strategy",
                &format!("strategy {} reset to Ready on startup (no auto-restart)", s.id),
                None,
            )
            .await?;
        }
    }
    repos::events::record_system_event(&db, SystemLevel::Info, "system", "boot complete", None).await?;

    // 4. In-memory market cache + provider.
    let cache = Arc::new(MarketCache::new(ProviderHealth::new("pending", true)));
    let provider: Arc<dyn MarketProvider> = match cfg.application.mode {
        AppMode::Demo => Arc::new(MockProvider::new()),
        AppMode::Replay => Arc::new(MockProvider::new()), // replay data is read from the DB
        AppMode::MarketData => match DexScreenerProvider::with_default_watchlist() {
            Ok(p) => Arc::new(p),
            Err(_) => {
                tracing::warn!("dexscreener provider failed to initialize; falling back to demo data");
                Arc::new(MockProvider::new())
            }
        },
    };
    tracing::info!(provider = provider.name(), simulated = provider.is_simulated(), "market provider chosen");

    // Shared event bus (WS delivery + internal notifications).
    let bus = EventBus::new(1024);

    // 5. Market data ingestion (spawns its own task).
    let (md_service, md_handle) = MarketDataService::new(
        db.clone(),
        bus.clone(),
        cache.clone(),
        provider.clone(),
        cfg.market_data.clone(),
    );

    // 6. Risk manager (hot cooldown state, pure rule evaluation).
    let risk = Arc::new(RiskManager::new(cfg.risk.clone()));

    // 7. Paper executor actor (single-writer for all trades).
    let (shutdown_tx, _shutdown_rx) = watch::channel(false);
    let exec = Executor::spawn(
        db.clone(),
        bus.clone(),
        cache.clone(),
        risk.clone(),
        (*cfg).clone(),
        account_id.clone(),
        sol_mint(&provider).to_string(),
        shutdown_tx.clone(),
    );

    // 8. Strategy lifecycle service.
    let strategies_svc = StrategyService::spawn(
        db.clone(),
        bus.clone(),
        cache.clone(),
        exec.clone(),
        md_handle.trigger.clone(),
        (*cfg).clone(),
        account_id.clone(),
        sol_mint(&provider).to_string(),
    );

    // 9. Equity sampler: fold account + cache into persistable equity series.
    let sampler_cfg = Arc::clone(&cache);
    let sampler_account = account_id.clone();
    let sampler_db = db.clone();
    let sampler_exec = exec.clone();
    let sampler_bus = bus.clone();
    let sampler_sol = sol_mint(&provider).to_string();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            if let Err(e) = equity_tick(
                &sampler_db,
                &sampler_exec,
                &sampler_cfg,
                &sampler_bus,
                &sampler_account,
                &sampler_sol,
            )
            .await
            {
                tracing::warn!(error = %e, "equity sampler tick failed");
            }
        }
    });

    // 10. Limit-order poller.
    let poller_db = db.clone();
    let poller_exec = exec.clone();
    let poller_account = account_id.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let open_limits = repos::trading::open_limit_orders(&poller_db, &poller_account)
                .await
                .unwrap_or_default();
            for order in open_limits {
                let _ = poller_exec.poll_limit(&order.id).await;
            }
        }
    });

    // 11. Shutdown coordinator.
    let (shutdown_tx2, shutdown_rx2) = watch::channel(false);
    {
        let shutdown_tx2_inner = shutdown_tx2.clone();
        let cfg_inner = cfg.clone();
        let account_inner = account_id.clone();
        let db_inner = db.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("shutdown requested");
            shutdown_tx2_inner.send(true).ok();
            // Give the executor a moment to drain.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let _ = (cfg_inner, account_inner, db_inner);
        });
    }

    let real_cfg = cfg.real.clone();
    let state = AppState {
        cfg,
        db: db.clone(),
        bus,
        cache,
        exec,
        strategies: strategies_svc,
        market_data: md_handle,
        real: Arc::new(crate::real::RealService::new(real_cfg)),
        rulebook: Arc::new(crate::agent::RulebookService::default()),
        account_id,
        sol_mint: sol_mint(&provider).to_string(),
        started_at,
        shutdown: shutdown_tx,
    };

    let listener = tokio::net::TcpListener::bind(format!(
        "{}:{}",
        state.cfg.application.bind_address, state.cfg.application.port
    ))
    .await
    .map_err(|e| AppError::Internal(format!("bind failed: {e}")))?;
    tracing::info!(
        address = %listener.local_addr().unwrap(),
        "http + websocket server listening"
    );

    // Start market ingestion.
    tokio::spawn(md_service.run());

    let app_router = api::router(state.clone());
    let mut shutdown_local = shutdown_rx2.clone();
    axum::serve(listener, app_router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_local.changed().await;
        })
        .await
        .map_err(|e| AppError::Internal(format!("server error: {e}")))?;

    tracing::info!("draining services");
    // Stop strategies, then drain the executor (single-writer), then close DB.
    state.strategies.stop_all().await;
    state.exec.drain().await;
    let _ = state.market_data.shutdown_tx.send(true);
    db.close().await?;
    tracing::info!("shutdown complete");
    Ok(())
}

/// Which SOL mint this provider prices things in (auto-deref through the Arc).
fn sol_mint(provider: &Arc<dyn MarketProvider>) -> &'static str {
    provider.sol_mint()
}

async fn equity_tick(
    db: &Database,
    _exec: &ExecutorHandle,
    cache: &MarketCache,
    bus: &EventBus,
    account_id: &str,
    sol_mint: &str,
) -> Result<(), AppError> {
    let account = repos::accounts::get_account(db, account_id).await?;
    let positions = repos::trading::list_positions(db, account_id).await?;
    let snaps = cache.latest_all().await;
    let marks = portfolio::marks_from_snapshots(&snaps, sol_mint);
    let val = portfolio::equity(&account, &positions, &marks);
    let snap = EquitySnapshot {
        id: crate::ids::new_id(),
        account_id: account_id.into(),
        ts: now_micros(),
        cash_lamports: account.cash_balance_lamports,
        positions_value_usd: val.positions_value_usd,
        unrealized_pnl_usd: val.unrealized_pnl_usd,
        equity_usd: val.equity_usd,
        realized_pnl_lamports: account.realized_pnl_lamports,
        exposure_pct: val.exposure_pct,
    };
    repos::accounts::insert_equity_snapshot(db, &snap).await?;
    bus.emit(WsEvent::Equity(snap));
    Ok(())
}