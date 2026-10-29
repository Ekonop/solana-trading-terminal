//! Market-data ingestion service: polls the configured provider, validates,
//! persists, updates the in-memory cache and provider health, and notifies
//! strategies. Runs once per process; a provider outage degrades health
//! visibly and never crashes the service or the API.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use crate::config::MarketDataConfig;
use crate::db::Database;
use crate::events::{EventBus, WsEvent};
use crate::market_data::{
    cache::MarketCache, providers::{fetch_with_retry, MarketProvider},
};
use crate::repos;
use crate::time::now_micros;

pub struct MarketDataService {
    cache: Arc<MarketCache>,
    db: Database,
    bus: EventBus,
    provider: Arc<dyn MarketProvider>,
    cfg: MarketDataConfig,
    trigger_tx: watch::Sender<u64>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

#[derive(Clone)]
pub struct MarketDataServiceHandle {
    pub cache: Arc<MarketCache>,
    /// Wakes strategies on new batches (increments each cycle).
    pub trigger: watch::Receiver<u64>,
    pub shutdown_tx: watch::Sender<bool>,
}

impl MarketDataService {
    pub fn new(
        db: Database,
        bus: EventBus,
        cache: Arc<MarketCache>,
        provider: Arc<dyn MarketProvider>,
        cfg: MarketDataConfig,
    ) -> (MarketDataService, MarketDataServiceHandle) {
        let (trigger_tx, trigger) = watch::channel(0u64);
        let (shutdown_tx, _) = watch::channel(false);
        let svc = MarketDataService {
            cache,
            db,
            bus,
            provider,
            cfg,
            trigger_tx,
            shutdown_tx,
        };
        let handle = MarketDataServiceHandle {
            cache: svc.cache.clone(),
            trigger,
            shutdown_tx: svc.shutdown_tx.clone(),
        };
        (svc, handle)
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    pub async fn run(self) {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(self.cfg.refresh_interval_ms));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_state = None;

        tracing::info!(
            provider = %self.provider.name(),
            simulated = self.provider.is_simulated(),
            "market-data service started"
        );

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("market-data service stopped");
                        break;
                    }
                }
                _ = interval.tick() => {
                    self.cycle().await;
                    let health = self.cache.health().await;
                    if last_state != Some(health.state) {
                        self.bus.emit(WsEvent::Health {
                            provider: health.provider.clone(),
                            healthy: health.is_up(),
                            provider_up: health.is_up(),
                            stale: !health.is_up(),
                        });
                        tracing::info!(
                            provider = %health.provider,
                            state = ?health.state,
                            "provider health changed"
                        );
                        last_state = Some(health.state);
                    }
                    // Wake strategy loops.
                    // NOTE: must NOT write `send(borrow().wrapping_add(1))` in a
                    // single expression — the temporary Ref holds the channel's
                    // read lock until the end of the statement, so send()'s
                    // internal write lock self-deadlocks (parking_lot
                    // wait_for_readers). Split it.
                    let v = self.trigger_tx.borrow().wrapping_add(1);
                    let _ = self.trigger_tx.send(v);
                }
            }
        }
    }

    async fn cycle(&self) {
                match fetch_with_retry(
            self.provider.as_ref(),
            self.cfg.max_retries,
            self.cfg.backoff_base_ms,
        )
        .await
        {
            Ok(quotes) => {
                                let now = now_micros();
                let mut ingested = 0usize;
                let mut first_ts = now;
                let _t0 = now_micros();
                                for q in &quotes {
                    let snap = crate::market_data::models::quote_to_snapshot(
                        q,
                        self.provider.name(),
                        self.provider.is_simulated(),
                        now,
                    );
                                        if let Err(e) = repos::markets::insert_snapshot(&self.db, &snap).await {
                        tracing::error!(error = %e, "failed to persist market snapshot");
                        continue;
                    }
                    // token metadata (untrusted, stored raw)
                                                            if let Err(e) = repos::events::upsert_token_meta(
                        &self.db,
                        &q.mint,
                        q.symbol.as_deref(),
                        q.name.as_deref(),
                        q.decimals,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, "token meta upsert failed");
                    }
                                        self.cache.insert(snap.clone()).await;
                                        self.bus.emit(WsEvent::Snapshot(snap));
                    ingested += 1;
                    first_ts = first_ts.min(q.observed_at);
                }
                self.cache
                    .set_health({
                        let mut h = self.cache.health().await;
                        h.record_success(now);
                        h
                    })
                    .await;
                                self.bus.emit(WsEvent::MarketBatch {
                    count: ingested,
                    simulated: self.provider.is_simulated(),
                    observed_at: first_ts,
                });
                tracing::debug!(ingested, "market batch ingested");
                            }
            Err(e) => {
                let now = now_micros();
                self.cache
                    .set_health({
                        let mut h = self.cache.health().await;
                        h.record_failure(now, &e.to_string());
                        h
                    })
                    .await;
                let health = self.cache.health().await;
                tracing::warn!(
                    provider = %self.provider.name(),
                    failures = health.consecutive_failures,
                    error = %e,
                    "market provider fetch failed"
                );
            }
        }
    }
}