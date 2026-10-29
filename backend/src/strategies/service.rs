//! Strategy lifecycle service: start/pause/stop/config, run persistence,
//! and fault isolation. Each running strategy gets its own tokio task; a
//! crash or evaluation error in one strategy never affects the others or the
//! API server.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, watch};
use tracing::{info, warn};

use crate::config::Config;
use crate::db::Database;
use crate::domain::{
    RunMode, SignalDecision, SignalRecord, StrategyState, SystemLevel,
};
use crate::errors::AppError;
use crate::events::EventBus;
use crate::ids;
use crate::market_data::cache::MarketCache;
use crate::repos;
use crate::trading::{ExecutorHandle, SubmitOutcome};
use crate::time::now_micros;

use super::registry;
use super::traits::{EvalContext, Strategy};

/// How the user wants a strategy to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Evaluate and submit paper orders.
    Paper,
    /// Evaluate and record signals only — nothing reaches the order engine.
    Shadow,
}

#[derive(Debug, Clone)]
pub struct StartRequest {
    pub strategy_id: String,
    pub run_mode: RunMode,
    pub execution: ExecutionMode,
}

enum Cmd {
    Start {
        req: StartRequest,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    Pause {
        strategy_id: String,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    Stop {
        strategy_id: String,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    UpdateConfig {
        strategy_id: String,
        config_json: String,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    StopAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Desired {
    Running,
    Paused,
    Stopped,
}

struct ActiveRun {
    run_id: String,
    desired: watch::Sender<Desired>,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
pub struct StrategyServiceHandle {
    tx: mpsc::Sender<Cmd>,
}

impl StrategyServiceHandle {
    pub async fn start(&self, strategy_id: &str, run_mode: RunMode, execution: ExecutionMode) -> Result<(), AppError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Start {
                req: StartRequest {
                    strategy_id: strategy_id.into(),
                    run_mode,
                    execution,
                },
                reply,
            })
            .await
            .map_err(|_| AppError::Lifecycle("strategy service is shut down".into()))?;
        rx.await
            .map_err(|_| AppError::Internal("strategy service crashed".into()))?
    }

    pub async fn pause(&self, strategy_id: &str) -> Result<(), AppError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Pause {
                strategy_id: strategy_id.into(),
                reply,
            })
            .await
            .map_err(|_| AppError::Lifecycle("strategy service is shut down".into()))?;
        rx.await
            .map_err(|_| AppError::Internal("strategy service crashed".into()))?
    }

    pub async fn stop(&self, strategy_id: &str) -> Result<(), AppError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Stop {
                strategy_id: strategy_id.into(),
                reply,
            })
            .await
            .map_err(|_| AppError::Lifecycle("strategy service is shut down".into()))?;
        rx.await
            .map_err(|_| AppError::Internal("strategy service crashed".into()))?
    }

    pub async fn update_config(&self, strategy_id: &str, config_json: &str) -> Result<(), AppError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Cmd::UpdateConfig {
                strategy_id: strategy_id.into(),
                config_json: config_json.into(),
                reply,
            })
            .await
            .map_err(|_| AppError::Lifecycle("strategy service is shut down".into()))?;
        rx.await
            .map_err(|_| AppError::Internal("strategy service crashed".into()))?
    }

    pub async fn stop_all(&self) {
        let _ = self.tx.send(Cmd::StopAll).await;
    }
}

pub struct StrategyService {
    db: Database,
    bus: EventBus,
    cache: Arc<MarketCache>,
    executor: ExecutorHandle,
    trigger: watch::Receiver<u64>,
    cfg: Config,
    account_id: String,
    sol_mint: String,
}

impl StrategyService {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        db: Database,
        bus: EventBus,
        cache: Arc<MarketCache>,
        executor: ExecutorHandle,
        trigger: watch::Receiver<u64>,
        cfg: Config,
        account_id: String,
        sol_mint: String,
    ) -> StrategyServiceHandle {
        let (tx, mut rx) = mpsc::channel::<Cmd>(64);
        let svc = StrategyService {
            db,
            bus,
            cache,
            executor,
            trigger,
            cfg,
            account_id,
            sol_mint,
        };
        tokio::spawn(async move {
            svc.run(&mut rx).await;
        });
        StrategyServiceHandle { tx }
    }

    async fn run(self, rx: &mut mpsc::Receiver<Cmd>) {
        let mut active: HashMap<String, ActiveRun> = HashMap::new();
        info!("strategy service started");

        while let Some(cmd) = rx.recv().await {
            match cmd {
                Cmd::Start { req, reply } => {
                    let res = self.start_one(&req, &mut active).await;
                    let _ = reply.send(res);
                }
                Cmd::Pause { strategy_id, reply } => {
                    let res = match active.get(&strategy_id) {
                        Some(run) => {
                            let _ = run.desired.send(Desired::Paused);
                            persist_status(&self.db, &strategy_id, StrategyState::Paused).await;
                            self.bus.emit(crate::events::WsEvent::Strategy {
                                id: strategy_id.clone(),
                                status: "paused".into(),
                            });
                            info!(strategy = %strategy_id, "strategy paused");
                            Ok(())
                        }
                        None => Err(AppError::Lifecycle(format!(
                            "strategy {strategy_id} is not running"
                        ))),
                    };
                    let _ = reply.send(res);
                }
                Cmd::Stop { strategy_id, reply } => {
                    let res = self.stop_one(&strategy_id, &mut active).await;
                    let _ = reply.send(res);
                }
                Cmd::UpdateConfig {
                    strategy_id,
                    config_json,
                    reply,
                } => {
                    let res = self.update_config_one(&strategy_id, &config_json, &active).await;
                    let _ = reply.send(res);
                }
                Cmd::StopAll => {
                    let ids: Vec<String> = active.keys().cloned().collect();
                    for id in ids {
                        if let Err(e) = self.stop_one(&id, &mut active).await {
                            warn!(strategy = %id, error = %e, "stop_all failed for strategy");
                        }
                    }
                }
            }
        }
        info!("strategy service stopped");
    }

    async fn start_one(
        &self,
        req: &StartRequest,
        active: &mut HashMap<String, ActiveRun>,
    ) -> Result<(), AppError> {
        if active.contains_key(&req.strategy_id) {
            return Err(AppError::Lifecycle(format!(
                "strategy {} is already running; pause or stop it first",
                req.strategy_id
            )));
        }
        let Some(strategy) = registry::find(&req.strategy_id) else {
            return Err(AppError::NotFound(format!("strategy {}", req.strategy_id)));
        };
        let row = repos::strategies::get_strategy(&self.db, &req.strategy_id).await?;
        let cfg: serde_json::Value = serde_json::from_str(&row.config_json)
            .map_err(|e| AppError::Strategy(format!("stored config invalid: {e}")))?;
        strategy.validate_config(&cfg)?;

        let run_id = ids::new_id();
        let run = crate::domain::StrategyRun {
            id: run_id.clone(),
            strategy_id: req.strategy_id.clone(),
            status: "running".into(),
            mode: req.run_mode,
            config_json: row.config_json.clone(),
            started_at: now_micros(),
            ended_at: None,
            error: None,
        };
        repos::strategies::start_run(&self.db, &run).await?;

        let status = if req.execution == ExecutionMode::Shadow {
            StrategyState::Shadow
        } else {
            StrategyState::PaperTrading
        };
        repos::strategies::set_strategy_status(&self.db, &req.strategy_id, status).await?;
        repos::strategies::set_strategy_error(&self.db, &req.strategy_id, None).await?;

        let (desired_tx, desired_rx) = watch::channel(Desired::Running);
        let inner = StrategyTask {
            db: self.db.clone(),
            bus: self.bus.clone(),
            cache: self.cache.clone(),
            executor: self.executor.clone(),
            trigger: self.trigger.clone(),
            cfg: self.cfg.clone(),
            account_id: self.account_id.clone(),
            sol_mint: self.sol_mint.clone(),
            strategy,
            config: cfg,
            run_id: run_id.clone(),
            execution: req.execution,
            strategy_id: req.strategy_id.clone(),
        };
        let handle_join = tokio::spawn(inner.run(desired_rx));

        active.insert(
            req.strategy_id.clone(),
            ActiveRun {
                run_id,
                desired: desired_tx,
                handle: handle_join,
            },
        );
        self.bus.emit(crate::events::WsEvent::Strategy {
            id: req.strategy_id.clone(),
            status: status.as_str().into(),
        });
        info!(
            strategy = %req.strategy_id,
            mode = ?req.run_mode,
            execution = ?req.execution,
            "strategy started"
        );
        Ok(())
    }

    async fn stop_one(
        &self,
        strategy_id: &str,
        active: &mut HashMap<String, ActiveRun>,
    ) -> Result<(), AppError> {
        let Some(run) = active.remove(strategy_id) else {
            return Err(AppError::Lifecycle(format!(
                "strategy {strategy_id} is not running"
            )));
        };
        let _ = run.desired.send(Desired::Stopped);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), run.handle).await;
        repos::strategies::end_run(&self.db, &run.run_id, "stopped", None).await?;
        repos::strategies::set_strategy_status(&self.db, strategy_id, StrategyState::Ready).await?;
        self.bus.emit(crate::events::WsEvent::Strategy {
            id: strategy_id.into(),
            status: "ready".into(),
        });
        info!(strategy = %strategy_id, "strategy stopped");
        Ok(())
    }

    async fn update_config_one(
        &self,
        strategy_id: &str,
        config_json: &str,
        active: &HashMap<String, ActiveRun>,
    ) -> Result<(), AppError> {
        if active.contains_key(strategy_id) {
            return Err(AppError::Lifecycle(format!(
                "strategy {strategy_id} is running; pause it before changing configuration"
            )));
        }
        let Some(strategy) = registry::find(strategy_id) else {
            return Err(AppError::NotFound(format!("strategy {strategy_id}")));
        };
        let cfg: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| AppError::Validation {
                field: "config".into(),
                message: format!("invalid JSON: {e}"),
            })?;
        strategy.validate_config(&cfg)?;
        repos::strategies::update_strategy_config(&self.db, strategy_id, config_json).await?;
        info!(strategy = %strategy_id, "strategy configuration updated");
        Ok(())
    }
}

/// The per-strategy evaluation task.
struct StrategyTask {
    db: Database,
    bus: EventBus,
    cache: Arc<MarketCache>,
    executor: ExecutorHandle,
    trigger: watch::Receiver<u64>,
    cfg: Config,
    account_id: String,
    sol_mint: String,
    strategy: &'static dyn Strategy,
    config: serde_json::Value,
    run_id: String,
    execution: ExecutionMode,
    strategy_id: String,
}

impl StrategyTask {
    async fn run(mut self, mut desired: watch::Receiver<Desired>) {
        info!(strategy = %self.strategy_id, "strategy task started");

        // Re-run the evaluation on every market batch until stopped.
        // `desired` holds the run state; we read it with a short borrow at the
        // top of each iteration and only hold the longer mutation while
        // awaiting a transition (watch::Receiver::changed is &mut).
        let mut last_trigger = *self.trigger.borrow();
        loop {
            let state = *desired.borrow();
            match state {
                Desired::Stopped => break,
                Desired::Paused => {
                    // Stay paused until the controller sends Running/Stopped.
                    if desired.changed().await.is_err() {
                        break;
                    }
                    continue;
                }
                Desired::Running => {}
            }

            let changed_now = tokio::select! {
                changed = desired.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    true
                }
                changed = self.trigger.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    false
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                    // Fallback tick: re-check state even without a trigger,
                    // so Pause/Stop always wins promptly.
                    false
                }
            };
            if changed_now {
                continue; // re-evaluate desired state at loop top
            }
            let now = *self.trigger.borrow();
            if now == last_trigger {
                continue;
            }
            last_trigger = now;
            if *desired.borrow() != Desired::Running {
                continue;
            }
            if let Err(e) = self.evaluate_once().await {
                warn!(strategy = %self.strategy_id, error = %e, "strategy evaluation failed");
                // Fault isolation: mark faulted, record the error,
                // stop the run. User can Start again after fixing.
                let _ = repos::strategies::set_strategy_status(
                    &self.db, &self.strategy_id, StrategyState::Faulted).await;
                let _ = repos::strategies::set_strategy_error(
                    &self.db, &self.strategy_id, Some(&e.to_string())).await;
                let _ = repos::strategies::end_run(
                    &self.db, &self.run_id, "faulted", Some(&e.to_string())).await;
                let _ = repos::events::record_system_event(
                    &self.db, SystemLevel::Error, "strategy",
                    &format!("{} faulted: {e}", self.strategy_id), None).await;
                self.bus.emit(crate::events::WsEvent::Strategy {
                    id: self.strategy_id.clone(),
                    status: "faulted".into(),
                });
                break;
            }
        }
        // Normal end: close the run row.
        let _ = repos::strategies::end_run(&self.db, &self.run_id, "stopped", None).await;
        info!(strategy = %self.strategy_id, "strategy task ended");
    }

    async fn evaluate_once(&self) -> Result<(), AppError> {
        let now = now_micros();
        let snaps = self.cache.latest_all().await;
        let positions = repos::trading::list_positions(&self.db, &self.account_id).await?;

        let ctx = EvalContext {
            strategy_id: &self.strategy_id,
            strategy_version: self.strategy.version(),
            now,
            sol_mint: &self.sol_mint,
            max_price_age_ms: self.cfg.market_data.maximum_data_age_ms,
            account_cash_lamports: 0, // filled below if needed by strategies
            equity_usd: risk_manager_equity_placeholder(
                &self.db,
                &self.account_id,
                &self.cache,
                &self.sol_mint,
            )
            .await,
            positions: &positions,
            snapshots: &snaps,
            cache: &self.cache,
        };

        let proposals = self.strategy.evaluate(&self.config, &ctx).await?;
        if proposals.is_empty() {
            repos::strategies::set_last_eval(&self.db, &self.strategy_id, now).await?;
            return Ok(());
        }

        for proposal in proposals {
            match self.execution {
                ExecutionMode::Shadow => {
                    // Shadow: record the signal as submitted with NO order.
                    let rec = SignalRecord {
                        id: ids::new_id(),
                        strategy_id: proposal.strategy_id.clone(),
                        strategy_version: proposal.strategy_version,
                        mint: proposal.mint.clone(),
                        direction: proposal.direction,
                        score: proposal.score,
                        reason: proposal.reason.clone(),
                        proposed_size: proposal.proposed_size,
                        decision: SignalDecision::Submitted,
                        rejection_reason: Some("shadow-mode: signal recorded, no paper order".into()),
                        data_observed_at: proposal.data_observed_at,
                        generated_at: now,
                        order_id: None,
                    };
                    repos::strategies::insert_signal(&self.db, &rec).await?;
                    self.bus.emit(crate::events::WsEvent::Signal(rec));
                }
                ExecutionMode::Paper => {
                    match self.executor.submit(&proposal).await {
                        Ok(outcome) => {
                            // Signals are persisted by the executor; only log.
                            match outcome {
                                SubmitOutcome::Filled { order, .. } => {
                                    tracing::debug!(strategy = %self.strategy_id,
                                        order = %order.id, "paper order filled");
                                }
                                SubmitOutcome::Rejected { reason, .. } => {
                                    tracing::debug!(strategy = %self.strategy_id,
                                        reason, "paper order rejected by executor");
                                }
                                SubmitOutcome::Pending { order } => {
                                    tracing::debug!(strategy = %self.strategy_id,
                                        order = %order.id, "paper order pending");
                                }
                            }
                        }
                        Err(e) => {
                            // The executor already recorded the signal + risk
                            // event on rejections; surface errors in logs.
                            warn!(strategy = %self.strategy_id, mint = proposal.mint,
                                  error = %e, "proposal not executed");
                        }
                    }
                }
            }
        }
        repos::strategies::set_last_eval(&self.db, &self.strategy_id, now).await?;
        Ok(())
    }
}

async fn persist_status(db: &Database, id: &str, status: StrategyState) {
    let _ = repos::strategies::set_strategy_status(db, id, status).await;
}

/// Small placeholder used by EvalContext (equity is displayed, not used for
/// strategy decisions beyond sizing which uses its own config fraction).
pub async fn risk_manager_equity_placeholder(
    db: &Database,
    account_id: &str,
    cache: &MarketCache,
    sol_mint: &str,
) -> rust_decimal::Decimal {
    match repos::accounts::get_account(db, account_id).await {
        Ok(account) => {
            let positions = repos::trading::list_positions(db, account_id)
                .await
                .unwrap_or_default();
            let marks = crate::trading::portfolio::marks_from_snapshots(
                &cache.latest_all().await,
                sol_mint,
            );
            crate::trading::portfolio::equity(&account, &positions, &marks).equity_usd
        }
        Err(_) => rust_decimal::Decimal::ZERO,
    }
}