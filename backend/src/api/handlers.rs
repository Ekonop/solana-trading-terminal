//! REST handlers. Thin by design — business logic lives in services and
//! repositories.

use axum::extract::{Path, Query, State};
use axum::Json;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::api::error::ApiResult;
use crate::api::query_limit;
use crate::app::AppState;
use crate::domain::*;
use crate::errors::AppError;
use crate::repos::{self, PageOut};
use crate::time;
use crate::trading::models::math::{lamports_to_usd, raw_to_human};

// ---------------------------------------------------------------------------
// Health / status
// ---------------------------------------------------------------------------

pub async fn health(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let db_ok = st.db.verify().await.is_ok();
    let cache_health = st.cache.health().await;
    Ok(Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "version": st.version(),
        "mode": st.cfg.application.mode.to_string(),
        "provider": cache_health.provider,
        "provider_healthy": cache_health.is_up(),
        "database": if db_ok { "ok" } else { "error" },
        "uptime_seconds": (time::now_micros() - st.started_at) / 1_000_000,
    })))
}

pub async fn status(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let health = st.cache.health().await;
    Ok(Json(serde_json::json!({
        "mode": st.cfg.application.mode.to_string(),
        "version": st.version(),
        "started_at": st.started_at,
        "simulated_data": st.cfg.application.mode == crate::config::AppMode::Demo,
        "provider": {
            "name": health.provider,
            "state": format!("{:?}", health.state).to_lowercase(),
            "simulated": health.simulated,
            "last_success": health.last_success,
            "consecutive_failures": health.consecutive_failures,
            "last_error": health.last_error,
        },
        "database": {
            "initialized": true,
            "data_dir": st.cfg.database.data_directory.file_name()
                .and_then(|s| s.to_str()).unwrap_or(".").to_string(),
        },
        "account_id": st.account_id,
    })))
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PositionView {
    #[serde(flatten)]
    pub position: Position,
    pub mark_price: Option<Decimal>,
    pub market_value_usd: Option<Decimal>,
    pub unrealized_usd: Option<Decimal>,
    pub value_available: bool,
}

pub async fn dashboard(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let account = repos::accounts::get_account(&st.db, &st.account_id).await?;
    let positions = repos::trading::list_positions(&st.db, &st.account_id).await?;
    let snaps = st.cache.latest_all().await;
    let marks = crate::trading::portfolio::marks_from_snapshots(&snaps, &st.sol_mint);
    let val = crate::trading::portfolio::equity(&account, &positions, &marks);

    let position_views: Vec<PositionView> = positions
        .iter()
        .filter(|p| p.qty_raw > 0)
        .map(|p| {
            let price = marks.price_of(&p.mint);
            let human = raw_to_human(p.qty_raw, 9);
            PositionView {
                position: p.clone(),
                mark_price: price,
                market_value_usd: price.map(|pr| human * pr),
                unrealized_usd: price.map(|pr| human * (pr - p.avg_entry_price)),
                value_available: price.is_some(),
            }
        })
        .collect();

    let series = repos::accounts::equity_series(&st.db, &st.account_id, 300).await?;
    let stats = crate::analytics::metrics::equity_stats(&series);
    let strategies = repos::strategies::list_strategies(&st.db).await?;
    let mut strat_views = Vec::new();
    for s in strategies {
        let total = repos::strategies::count_signals(&st.db, &s.id, None).await?;
        let rejected = repos::strategies::count_signals(
            &st.db,
            &s.id,
            Some(SignalDecision::Rejected),
        )
        .await?;
        strat_views.push(serde_json::json!({
            "id": s.id,
            "display_name": s.display_name,
            "status": s.status.as_str(),
            "config_version": s.config_version,
            "last_eval_at": s.last_eval_at,
            "last_error": s.last_error,
            "signals_total": total,
            "signals_rejected": rejected,
        }));
    }

    let latest_signals = repos::strategies::list_signals(&st.db, None, crate::repos::Page::new(Some(10), None))
        .await?
        .items;
    let latest_risk = repos::events::latest_risk_events(&st.db, 10).await?;
    let recent_fills = repos::trading::list_fills(&st.db, &st.account_id, crate::repos::Page::new(Some(5), None))
        .await?
        .items;
    let recent_orders = repos::trading::list_orders(&st.db, &st.account_id, None, crate::repos::Page::new(Some(5), None))
        .await?
        .items;

    let health = st.cache.health().await;
    Ok(Json(serde_json::json!({
        "account": account,
        "equity": {
            "cash_lamports": val.cash_lamports,
            "cash_usd": lamports_to_usd(val.cash_lamports, val.solusd),
            "positions_value_usd": val.positions_value_usd,
            "equity_usd": val.equity_usd,
            "unrealized_pnl_usd": val.unrealized_pnl_usd,
            "realized_pnl_lamports": account.realized_pnl_lamports,
            "realized_pnl_usd": lamports_to_usd(account.realized_pnl_lamports, val.solusd),
            "exposure_pct": val.exposure_pct,
            "solusd": val.solusd,
        },
        "stats": stats,
        "positions": position_views,
        "open_positions": position_views.len(),
        "market_health": {
            "provider": health.provider,
            "state": format!("{:?}", health.state).to_lowercase(),
            "simulated": health.simulated,
            "last_success": health.last_success,
            "last_error": health.last_error,
        },
        "strategies": strat_views,
        "latest_signals": latest_signals,
        "latest_risk_events": latest_risk,
        "recent_fills": recent_fills,
        "recent_orders": recent_orders,
        "series": series,
        "simulated": health.simulated,
    })))
}

// ---------------------------------------------------------------------------
// Portfolio / positions
// ---------------------------------------------------------------------------

pub async fn portfolio(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let account = repos::accounts::get_account(&st.db, &st.account_id).await?;
    let positions = repos::trading::list_positions(&st.db, &st.account_id).await?;
    let snaps = st.cache.latest_all().await;
    let marks = crate::trading::portfolio::marks_from_snapshots(&snaps, &st.sol_mint);
    let val = crate::trading::portfolio::equity(&account, &positions, &marks);
    let series = repos::accounts::equity_series(&st.db, &st.account_id, 300).await?;
    let stats = crate::analytics::metrics::equity_stats(&series);
    Ok(Json(serde_json::json!({
        "account": account,
        "equity": val,
        "stats": stats,
        "series": series,
    })))
}

pub async fn positions(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let positions = repos::trading::list_positions(&st.db, &st.account_id).await?;
    let snaps = st.cache.latest_all().await;
    let marks = crate::trading::portfolio::marks_from_snapshots(&snaps, &st.sol_mint);
    let mut views = Vec::new();
    for p in positions.iter().filter(|p| p.qty_raw > 0) {
        let price = marks.price_of(&p.mint);
        let human = raw_to_human(p.qty_raw, 9);
        views.push(serde_json::json!({
            "mint": p.mint,
            "symbol": snaps.iter().find(|s| s.mint == p.mint).and_then(|s| s.symbol.clone()),
            "qty_raw": p.qty_raw,
            "qty_human": human,
            "avg_entry_price": p.avg_entry_price,
            "mark_price": price,
            "market_value_usd": price.map(|pr| human * pr),
            "unrealized_pnl_usd": price.map(|pr| human * (pr - p.avg_entry_price)),
            "fees_paid_lamports": 0, // derivable from ledger; kept explicit in UI via ledger
            "opened_at": p.opened_at,
            "updated_at": p.updated_at,
            "age_seconds": (time::now_micros() - p.opened_at) / 1_000_000,
            "value_available": price.is_some(),
        }));
    }
    Ok(Json(serde_json::json!({ "positions": views })))
}

// ---------------------------------------------------------------------------
// Trading history
// ---------------------------------------------------------------------------

pub async fn orders(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<PageOut<PaperOrder>>> {
    let page = query_limit(q.get("limit").cloned(), q.get("offset").cloned());
    let status = q.get("status").and_then(|s| match s.as_str() {
        "open" => Some(OrderStatus::Open),
        "filled" => Some(OrderStatus::Filled),
        "cancelled" => Some(OrderStatus::Cancelled),
        "rejected" => Some(OrderStatus::Rejected),
        _ => None,
    });
    let out = repos::trading::list_orders(&st.db, &st.account_id, status, page).await?;
    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SubmitOrderReq {
    pub mint: String,
    pub side: String, // "buy" | "sell"
    /// Size in USD (decimal string or number).
    pub size_usd: Decimal,
    pub order_type: Option<String>,
    pub limit_price: Option<Decimal>,
}

pub async fn submit_order(
    State(st): State<AppState>,
    Json(req): Json<SubmitOrderReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if req.size_usd <= Decimal::ZERO {
        return Err(AppError::Validation {
            field: "size_usd".into(),
            message: "must be > 0".into(),
        });
    }
    let side = match req.side.as_str() {
        "buy" => Direction::Buy,
        "sell" => Direction::Sell,
        _ => {
            return Err(AppError::Validation {
                field: "side".into(),
                message: "must be 'buy' or 'sell'".into(),
            })
        }
    };
    let Some(snap) = st.cache.latest_for(&req.mint).await else {
        return Err(AppError::MarketData(format!(
            "no market snapshot for {}",
            req.mint
        )));
    };
    let size_cents = (req.size_usd * Decimal::from(100)).round_dp(0).to_i64().unwrap_or(0);
    let proposal = SignalProposal {
        strategy_id: "manual".into(),
        strategy_version: 0,
        mint: req.mint.clone(),
        market_id: snap.pair_id.clone(),
        direction: side,
        score: None,
        reason: "manual paper order from the terminal UI".into(),
        proposed_size: Some(size_cents),
        ref_price: snap.price,
        data_observed_at: snap.observed_at,
        snapshot_id: Some(snap.id.clone()),
    };
    let outcome = st.exec.submit(&proposal).await?;
    let order = outcome.order().clone();
    let fills = repos::trading::fills_for_order(&st.db, &order.id).await?;
    Ok(Json(serde_json::json!({
        "order": order,
        "fills": fills,
        "outcome": match outcome {
            crate::trading::SubmitOutcome::Filled { .. } => "filled",
            crate::trading::SubmitOutcome::Rejected { .. } => "rejected",
            crate::trading::SubmitOutcome::Pending { .. } => "pending",
        },
    })))
}

pub async fn cancel_order(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<PaperOrder>> {
    let order = repos::trading::get_order(&st.db, &id).await?;
    if order.status != OrderStatus::Open {
        return Err(AppError::Conflict(format!(
            "order {id} is {:?}, not cancellable",
            order.status
        )));
    }
    let tx = st.db.transaction().await?;
    repos::trading::set_order_status(&tx, &id, OrderStatus::Cancelled, None, Some("cancelled by user"))
        .await?;
    tx.commit().await?;
    let updated = repos::trading::get_order(&st.db, &id).await?;
    st.bus.emit(crate::events::WsEvent::Order(updated.clone()));
    Ok(Json(updated))
}

pub async fn fills(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<PageOut<Fill>>> {
    let page = query_limit(q.get("limit").cloned(), q.get("offset").cloned());
    let out = repos::trading::list_fills(&st.db, &st.account_id, page).await?;
    Ok(Json(out))
}

pub async fn ledger(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<PageOut<LedgerEntry>>> {
    let page = query_limit(q.get("limit").cloned(), q.get("offset").cloned());
    let out = repos::accounts::list_ledger(&st.db, &st.account_id, page).await?;
    Ok(Json(out))
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

pub async fn strategies_list(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let strategies = repos::strategies::list_strategies(&st.db).await?;
    let mut out = Vec::new();
    for s in strategies {
        let total = repos::strategies::count_signals(&st.db, &s.id, None).await?;
        let rejected =
            repos::strategies::count_signals(&st.db, &s.id, Some(SignalDecision::Rejected)).await?;
        out.push(serde_json::json!({
            "id": s.id,
            "display_name": s.display_name,
            "description": s.description,
            "status": s.status.as_str(),
            "enabled": s.enabled,
            "config": serde_json::from_str::<serde_json::Value>(&s.config_json).unwrap_or(serde_json::Value::Null),
            "config_version": s.config_version,
            "last_eval_at": s.last_eval_at,
            "last_error": s.last_error,
            "signals_total": total,
            "signals_rejected": rejected,
        }));
    }
    Ok(Json(serde_json::json!({ "strategies": out })))
}

pub async fn strategy_detail(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let s = repos::strategies::get_strategy(&st.db, &id).await?;
    let runs = repos::strategies::list_runs(&st.db, Some(&id), crate::repos::Page::new(Some(20), None)).await?;
    let signals = repos::strategies::list_signals(&st.db, Some(&id), crate::repos::Page::new(Some(50), None)).await?;
    let orders = repos::trading::list_orders(&st.db, &st.account_id, None, crate::repos::Page::new(Some(20), None)).await?;
    Ok(Json(serde_json::json!({
        "strategy": s,
        "runs": runs.items,
        "signals": signals.items,
        "recent_orders": orders.items,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StartReq {
    pub mode: Option<String>,
    pub execution: Option<String>,
}

pub async fn strategy_start(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<StartReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let run_mode = match req.mode.as_deref().unwrap_or("demo") {
        "demo" => RunMode::Demo,
        "live" => RunMode::Live,
        "replay" => RunMode::Replay,
        other => {
            return Err(AppError::Validation {
                field: "mode".into(),
                message: format!("unknown mode {other}; use demo|live|replay"),
            })
        }
    };
    let execution = match req.execution.as_deref().unwrap_or("paper") {
        "paper" => crate::strategies::service::ExecutionMode::Paper,
        "shadow" => crate::strategies::service::ExecutionMode::Shadow,
        other => {
            return Err(AppError::Validation {
                field: "execution".into(),
                message: format!("unknown execution {other}; use paper|shadow"),
            })
        }
    };
    st.strategies
        .start(&id, run_mode, execution)
        .await?;
    let s = repos::strategies::get_strategy(&st.db, &id).await?;
    Ok(Json(serde_json::json!({ "status": s.status.as_str() })))
}

pub async fn strategy_pause(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    st.strategies.pause(&id).await?;
    let s = repos::strategies::get_strategy(&st.db, &id).await?;
    Ok(Json(serde_json::json!({ "status": s.status.as_str() })))
}

pub async fn strategy_stop(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    st.strategies.stop(&id).await?;
    let s = repos::strategies::get_strategy(&st.db, &id).await?;
    Ok(Json(serde_json::json!({ "status": s.status.as_str() })))
}

pub async fn strategy_update_config(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(cfg): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let json = serde_json::to_string(&cfg)
        .map_err(|e| AppError::Validation { field: "config".into(), message: e.to_string() })?;
    st.strategies.update_config(&id, &json).await?;
    let s = repos::strategies::get_strategy(&st.db, &id).await?;
    Ok(Json(serde_json::json!({ "config": serde_json::from_str::<serde_json::Value>(&s.config_json).unwrap_or_default() })))
}

// ---------------------------------------------------------------------------
// Signals (strategy decision feed)
// ---------------------------------------------------------------------------

pub async fn signals_list(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<PageOut<SignalRecord>>> {
    let strategy = q.get("strategy").map(|s| s.as_str());
    let page = query_limit(q.get("limit").cloned(), q.get("offset").cloned());
    let out = repos::strategies::list_signals(&st.db, strategy, page).await?;
    Ok(Json(out))
}

// ---------------------------------------------------------------------------
// Markets
// ---------------------------------------------------------------------------

pub async fn markets(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let snaps = st.cache.latest_all().await;
    let health = st.cache.health().await;
    let mut views = Vec::new();
    for s in &snaps {
        views.push(serde_json::json!({
            "mint": s.mint,
            "symbol": s.symbol,
            "name": s.name,
            "pair_id": s.pair_id,
            "price": s.price,
            "liquidity_usd": s.liquidity_usd,
            "volume_24h_usd": s.volume_24h_usd,
            "source": s.source,
            "is_simulated": s.is_simulated,
            "observed_at": s.observed_at,
            "quality": s.quality,
            "quality_label": s.quality_label(st.cfg.market_data.maximum_data_age_ms),
            "stale": s.is_stale(st.cfg.market_data.maximum_data_age_ms),
        }));
    }
    Ok(Json(serde_json::json!({
        "provider": health.provider,
        "state": format!("{:?}", health.state).to_lowercase(),
        "simulated": health.simulated,
        "last_success": health.last_success,
        "markets": views,
    })))
}

pub async fn market_detail(
    State(st): State<AppState>,
    Path(mint): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let latest = repos::markets::latest_for(&st.db, &mint).await?;
    let Some(latest) = latest else {
        return Err(AppError::NotFound(format!("market {mint}")));
    };
    let series = st
        .cache
        .history_for(&mint, 500)
        .await
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "ts": s.observed_at,
                "price": s.price,
                "volume_24h_usd": s.volume_24h_usd,
                "liquidity_usd": s.liquidity_usd,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({
        "market": latest,
        "series": series,
        "stale": latest.is_stale(st.cfg.market_data.maximum_data_age_ms),
    })))
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

pub async fn risk_events(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<PageOut<RiskEvent>>> {
    let page = query_limit(q.get("limit").cloned(), q.get("offset").cloned());
    let out = repos::events::list_risk_events(&st.db, page).await?;
    Ok(Json(out))
}

pub async fn system_events(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<PageOut<SystemEvent>>> {
    let page = query_limit(q.get("limit").cloned(), q.get("offset").cloned());
    let out = repos::events::list_system_events(&st.db, page).await?;
    Ok(Json(out))
}

// ---------------------------------------------------------------------------
// Backtests
// ---------------------------------------------------------------------------

pub async fn backtests_list(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<PageOut<BacktestRun>>> {
    let page = query_limit(q.get("limit").cloned(), q.get("offset").cloned());
    let out = repos::backtests::list_backtests(&st.db, page).await?;
    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BacktestReq {
    pub strategy_id: String,
    pub config: Option<serde_json::Value>,
    pub interval_start: Option<i64>,
    pub interval_end: Option<i64>,
    pub initial_lamports: Option<i64>,
    pub fee_bps: Option<i64>,
    pub slippage_bps: Option<i64>,
}

pub async fn backtests_create(
    State(st): State<AppState>,
    Json(req): Json<BacktestReq>,
) -> ApiResult<Json<BacktestRun>> {
    // Validate the strategy + config up front.
    let Some(strategy) = crate::strategies::registry::find(&req.strategy_id) else {
        return Err(AppError::NotFound(format!("strategy {}", req.strategy_id)));
    };
    let row = repos::strategies::get_strategy(&st.db, &req.strategy_id).await?;
    let config_json = match req.config {
        Some(c) => {
            strategy.validate_config(&c)?;
            serde_json::to_string(&c).map_err(|e| AppError::Validation {
                field: "config".into(),
                message: e.to_string(),
            })?
        }
        None => row.config_json.clone(),
    };
    // Determine a sane window: end = now, start as requested or 24h ago.
    let now = time::now_micros();
    let interval_end = req.interval_end.unwrap_or(now);
    let interval_start = req.interval_start.unwrap_or(now - 24 * 3_600 * 1_000_000);
    if interval_start >= interval_end {
        return Err(AppError::Validation {
            field: "interval_start".into(),
            message: "must be < interval_end".into(),
        });
    }
    let btreq = crate::backtesting::engine::BacktestRequest {
        strategy_id: req.strategy_id.clone(),
        config_json,
        interval_start,
        interval_end,
        initial_lamports: req.initial_lamports.unwrap_or(st.cfg.paper_trading.initial_balance_lamports),
        fee_bps: req.fee_bps.unwrap_or(st.cfg.paper_trading.fee_bps),
        slippage_bps: req.slippage_bps.unwrap_or(st.cfg.paper_trading.slippage_bps),
        sol_mint: st.sol_mint.clone(),
    };
    let run = crate::backtesting::engine::enqueue_backtest(&st.db, btreq).await?;
    Ok(Json(run))
}

pub async fn backtest_detail(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<BacktestRun>> {
    let run = repos::backtests::get_backtest(&st.db, &id).await?;
    Ok(Json(run))
}

// ---------------------------------------------------------------------------
// Settings / admin
// ---------------------------------------------------------------------------

pub async fn settings(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let health = st.cache.health().await;
    let auto = repos::events::get_setting(&st.db, "auto_start_strategies").await?;
    let persisted = repos::events::get_setting(&st.db, "market_data_enabled").await?;
    Ok(Json(serde_json::json!({
        "application": {
            "mode": st.cfg.application.mode.to_string(),
            "bind_address": st.cfg.application.bind_address,
            "port": st.cfg.application.port,
            "version": st.version(),
        },
        "paper_trading": {
            "initial_balance_lamports": st.cfg.paper_trading.initial_balance_lamports,
            "fee_bps": st.cfg.paper_trading.fee_bps,
            "slippage_bps": st.cfg.paper_trading.slippage_bps,
            "max_order_notional_usd": st.cfg.paper_trading.max_order_notional_usd,
        },
        "risk": {
            "max_position_fraction": st.cfg.risk.max_position_fraction,
            "max_total_exposure": st.cfg.risk.max_total_exposure,
            "max_daily_loss": st.cfg.risk.max_daily_loss,
            "max_drawdown": st.cfg.risk.max_drawdown,
            "max_open_positions": st.cfg.risk.max_open_positions,
            "minimum_liquidity_usd": st.cfg.risk.minimum_liquidity_usd,
            "minimum_volume_24h_usd": st.cfg.risk.minimum_volume_24h_usd,
            "max_slippage_bps": st.cfg.risk.max_slippage_bps,
            "max_price_age_ms": st.cfg.risk.max_price_age_ms,
            "per_token_cooldown_ms": st.cfg.risk.per_token_cooldown_ms,
            "max_liquidity_impact": st.cfg.risk.max_liquidity_impact,
            "stop_loss": st.cfg.risk.stop_loss,
            "take_profit": st.cfg.risk.take_profit,
        },
        "market_data": {
            "provider": st.cfg.market_data.provider.to_string(),
            "refresh_interval_ms": st.cfg.market_data.refresh_interval_ms,
            "maximum_data_age_ms": st.cfg.market_data.maximum_data_age_ms,
            "provider_state": format!("{:?}", health.state).to_lowercase(),
            "last_success": health.last_success,
        },
        "runtime": {
            "auto_start_strategies": auto.unwrap_or_else(|| "false".into()),
            "market_data_enabled": persisted.unwrap_or_else(|| "true".into()),
        },
    })))
}

pub async fn stop_all(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    st.strategies.stop_all().await;
    Ok(Json(serde_json::json!({ "stopped": true })))
}

#[derive(Debug, Deserialize)]
pub struct ResetReq {
    pub confirm: String,
}

pub async fn reset_demo_data(
    State(st): State<AppState>,
    Json(req): Json<ResetReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if req.confirm != "RESET" {
        return Err(AppError::Validation {
            field: "confirm".into(),
            message: "send {\"confirm\":\"RESET\"} to wipe simulated trading state".into(),
        });
    }
    // Stop every strategy first.
    st.strategies.stop_all().await;

    let tx = st.db.transaction().await?;
    let account = repos::accounts::get_account(&tx, &st.account_id).await?;
    for table in [
        "paper_fills",
        "paper_orders",
        "strategy_signals",
        "ledger_entries",
        "equity_snapshots",
        "positions",
        "risk_events",
    ] {
        tx.exec(&format!("DELETE FROM {table}"))
            .await
            .map_err(|e| AppError::Database(format!("reset failed on {table}: {e}")))?;
    }
    let now = time::now_micros();
    tx.exec(&format!(
        "UPDATE paper_accounts SET cash_balance_lamports = {}, realized_pnl_lamports = 0, updated_at = {} WHERE id = '{}'",
        account.initial_balance_lamports, now, st.account_id
    ))
    .await
    .map_err(|e| AppError::Database(format!("reset account: {e}")))?;
    tx.exec(&format!(
        "INSERT INTO ledger_entries (id, account_id, ts, kind, amount_lamports, balance_after, note)
         VALUES ('{}','{}',{},'initial_deposit',{},{},'demo data reset')",
        crate::ids::new_id(),
        st.account_id,
        now,
        account.initial_balance_lamports,
        account.initial_balance_lamports
    ))
    .await
    .map_err(|e| AppError::Database(format!("reset ledger: {e}")))?;
    // Re-enable strategies so the user can start them again.
    for sid in ["sniper", "momentum", "dip_hunter", "whale_tail", "scalper"] {
        let _ = repos::strategies::set_strategy_status(&tx, sid, crate::domain::StrategyState::Ready).await;
    }
    tx.commit().await?;
    repos::events::record_system_event(
        &st.db,
        crate::domain::SystemLevel::Info,
        "settings",
        "demo trading data reset by user",
        None,
    )
    .await?;
    Ok(Json(serde_json::json!({ "reset": true })))
}