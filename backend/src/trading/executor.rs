//! The paper executor: a single-writer actor owning all trade application.
//!
//! Why an actor? pglite already serializes every statement and transaction at
//! the engine level, but the paper engine also needs *ordered read-modify-
//! write sequences* (risk context assembly reads current positions/equity,
//! then the trade applies against a snapshot of that state). Routing every
//! "propose -> risk -> apply" sequence through one task guarantees:
//!   1. No two strategies race on the same cash/position.
//!   2. Bounded backpressure (the channel is bounded; callers await).
//!   3. A single place that can be shut down cleanly (drain) before the DB.
//!
//! Trades are applied inside `apply::insert_and_fill` within a single pglite
//! transaction; the engine serialization is a second, independent layer.

use std::sync::Arc;

use rust_decimal::Decimal;
use tokio::sync::{mpsc, watch};

use crate::config::Config;
use crate::db::Database;
use crate::domain::{
    Direction, EquitySnapshot, MarketSnapshot, OrderSide, OrderType, PaperOrder,
    SignalDecision, SignalProposal, SignalRecord, SystemLevel,
};
use crate::errors::AppError;
use crate::events::{EventBus, WsEvent};
use crate::ids;
use crate::market_data::cache::MarketCache;
use crate::repos;
use crate::risk::{RiskContext, RiskManager};
use crate::time::now_micros;
use crate::trading::apply::{insert_and_fill, FillContext};
use crate::trading::fees::FeeModel;
use crate::trading::models::OrderIntent;

/// Result of a submit through the actor.
#[derive(Debug, Clone)]
pub enum SubmitOutcome {
    Filled { order: PaperOrder, fill: crate::domain::Fill },
    Rejected { order: PaperOrder, reason: String },
    Pending { order: PaperOrder },
}

impl SubmitOutcome {
    pub fn order(&self) -> &PaperOrder {
        match self {
            SubmitOutcome::Filled { order, .. }
            | SubmitOutcome::Rejected { order, .. }
            | SubmitOutcome::Pending { order } => order,
        }
    }
}

enum Msg {
    /// Submit one validated proposal (after risk approval).
    Submit {
        proposal: SignalProposal,
        reply: tokio::sync::oneshot::Sender<Result<SubmitOutcome, AppError>>,
    },
    /// Kill-switch: stop processing further submits, drain pending, notify done.
    Drain {
        done: tokio::sync::oneshot::Sender<()>,
    },
    /// Record equity snapshot (fire-ish; no reply needed beyond result).
    Snapshot {
        snap: EquitySnapshot,
    },
    /// Service an open limit order against a new observation.
    PollLimit {
        order_id: String,
        reply: tokio::sync::oneshot::Sender<Result<Option<SubmitOutcome>, AppError>>,
    },
}

#[derive(Clone)]
pub struct ExecutorHandle {
    tx: mpsc::Sender<Msg>,
}

impl ExecutorHandle {
    pub async fn submit(
        &self,
        proposal: &SignalProposal,
    ) -> Result<SubmitOutcome, AppError> {
        let (reply, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(Msg::Submit {
                proposal: proposal.clone(),
                reply,
            })
            .await
            .map_err(|_| AppError::DatabaseUnavailable("executor is shut down".into()))?;
        rx.await
            .map_err(|_| AppError::DatabaseUnavailable("executor task crashed".into()))
            .and_then(|r| r)
    }

    pub async fn poll_limit(&self, order_id: &str) -> Result<Option<SubmitOutcome>, AppError> {
        let (reply, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(Msg::PollLimit {
                order_id: order_id.to_string(),
                reply,
            })
            .await
            .map_err(|_| AppError::DatabaseUnavailable("executor is shut down".into()))?;
        rx.await
            .map_err(|_| AppError::DatabaseUnavailable("executor task crashed".into()))
            .and_then(|r| r)
    }

    pub async fn record_equity(&self, snap: EquitySnapshot) -> Result<(), AppError> {
        // Best-effort; failures surface in system_events by the actor.
        let _ = self.tx.send(Msg::Snapshot { snap }).await;
        Ok(())
    }

    pub async fn drain(&self) {
        let (done, rx) = tokio::sync::oneshot::channel();
        if self.tx.send(Msg::Drain { done }).await.is_err() {
            return;
        }
        let _ = rx.await;
    }
}

/// The actor task. `account_id` is the single paper account.
pub struct Executor {
    db: Database,
    bus: EventBus,
    cache: Arc<MarketCache>,
    risk: Arc<RiskManager>,
    cfg: Config,
    account_id: String,
    sol_mint: String,
    shutdown_tx: watch::Sender<bool>,
}

impl Executor {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        db: Database,
        bus: EventBus,
        cache: Arc<MarketCache>,
        risk: Arc<RiskManager>,
        cfg: Config,
        account_id: String,
        sol_mint: String,
        shutdown_tx: watch::Sender<bool>,
    ) -> ExecutorHandle {
        let (tx, mut rx) = mpsc::channel::<Msg>(256);
        let executor = Executor {
            db,
            bus,
            cache,
            risk,
            cfg,
            account_id,
            sol_mint,
            shutdown_tx,
        };
        tokio::spawn(async move {
            executor.run(&mut rx).await;
        });
        ExecutorHandle { tx }
    }

    async fn run(self, rx: &mut mpsc::Receiver<Msg>) {
        let fee_model = FeeModel::from_config(&self.cfg.paper_trading);
        let slippage = self.cfg.paper_trading.slippage_bps;
        tracing::info!("paper executor actor started");

        while let Some(msg) = rx.recv().await {
            match msg {
                Msg::Drain { done } => {
                    // Kill-switch reached: stop accepting new work (already the
                    // case for the channel), emit remaining events, exit.
                    tracing::info!("paper executor draining");
                    let _ = done.send(());
                    continue;
                }
                Msg::Snapshot { snap } => {
                    if let Err(e) = repos::accounts::insert_equity_snapshot(&self.db, &snap).await {
                        tracing::error!(error = %e, "equity snapshot insert failed");
                        let _ = repos::events::record_system_event(
                            &self.db,
                            SystemLevel::Error,
                            "equity",
                            &format!("equity snapshot insert failed: {e}"),
                            None,
                        )
                        .await;
                    }
                    self.bus.emit(WsEvent::Equity(snap));
                }
                Msg::PollLimit { order_id, reply } => {
                    let res = self
                        .poll_limit_locked(&order_id, &fee_model, slippage)
                        .await;
                    let _ = reply.send(res);
                }
                Msg::Submit { proposal, reply } => {
                    let res = self
                        .submit_one(&proposal, &fee_model, slippage)
                        .await;
                    let _ = reply.send(res);
                }
            }
        }
        tracing::info!("paper executor actor stopped");
        let _ = self.shutdown_tx.send(true);
    }

    // -------- submit one proposal (the full pipeline) --------
    async fn submit_one(
        &self,
        proposal: &SignalProposal,
        fee_model: &FeeModel,
        slippage: i64,
    ) -> Result<SubmitOutcome, AppError> {
        let now = now_micros();

        // 1. Ignore hold proposals (shouldn't reach the executor anyway).
        if proposal.direction == Direction::Hold {
            let rec = persist_signal_rec(
                &self.db,
                proposal,
                SignalDecision::Rejected,
                Some("hold-signal-ignored".into()),
                None,
                now,
            )
            .await?;
            self.bus.emit(WsEvent::Signal(rec));
            return Err(AppError::RiskRejected("hold signal ignored".into()));
        }

        // 2. Snapshot for the mint must exist and be fresh enough to act on.
        let Some(snap) = self.cache.latest_for(&proposal.mint).await else {
            let rec = persist_signal_rec(
                &self.db,
                proposal,
                SignalDecision::Rejected,
                Some("no-market-data".into()),
                None,
                now,
            )
            .await?;
            self.bus.emit(WsEvent::Signal(rec));
            self.record_risk_event(proposal, "no-market-data", "no snapshot for mint", None)
                .await;
            return Err(AppError::MarketData(format!(
                "no snapshot for {}",
                proposal.mint
            )));
        };

        // 3. Central risk evaluation (every proposal must pass).
        let ctx = self.build_risk_context(proposal, &snap).await?;
        let decision = self.risk.evaluate(proposal, &ctx).await?;
        if !decision.approved {
            let rule = decision.rule.clone().unwrap_or_else(|| "risk".into());
            let rec = persist_signal_rec(
                &self.db,
                proposal,
                SignalDecision::Rejected,
                decision.detail.clone(),
                None,
                now,
            )
            .await?;
            self.bus.emit(WsEvent::Signal(rec));
            self.record_risk_event(proposal, &rule, &decision.detail.clone().unwrap_or_default(), None)
                .await;
            tracing::info!(
                strategy = proposal.strategy_id,
                mint = proposal.mint,
                rule,
                "signal rejected by risk manager"
            );
            return Err(AppError::RiskRejected(rule));
        }

        // 4. Apply the trade atomically.
        let solusd = self
            .cache
            .sol_price(&self.sol_mint)
            .await
            .unwrap_or_else(|| Decimal::from(150));
        let fill_ctx = FillContext {
            snapshot: snap.clone(),
            solusd,
            fee_model: *fee_model,
            slippage_bps: slippage,
        };
        let intent = OrderIntent {
            account_id: self.account_id.clone(),
            // Manual terminal orders use the "manual" sentinel (no strategies
            // row exists for it); persist NULL so the FK holds.
            strategy_id: (proposal.strategy_id != "manual")
                .then(|| proposal.strategy_id.clone()),
            mint: proposal.mint.clone(),
            side: match proposal.direction {
                Direction::Buy => OrderSide::Buy,
                Direction::Sell => OrderSide::Sell,
                Direction::Hold => unreachable!(),
            },
            order_type: OrderType::Market,
            size_usd_cents: proposal.proposed_size.unwrap_or(0),
            limit_price: None,
            idempotency_key: crate::ids::proposal_idempotency_key(proposal),
            correlation_id: ids::new_id(),
            ref_price: proposal.ref_price,
            ref_snapshot_id: proposal.snapshot_id.clone(),
            data_observed_at: proposal.data_observed_at,
        };
        let tx = self.db.transaction().await?;
        let outcome = insert_and_fill(&tx, &intent, &fill_ctx).await;

        match outcome {
            Ok(apply) => {
                // Commit FIRST so audit rows can reference the order; the
                // signal/risk rows are then recorded outside the trade tx
                // (a failure there is logged, never rolls back a valid fill).
                match apply {
                    crate::trading::apply::ApplyOutcome::Filled { order, fill } => {
                        let order_id = order.id.clone();
                        let fill_clone = fill.clone();
                        let order_clone = order.clone();
                        tx.commit().await?;
                        tracing::info!(
                            strategy = proposal.strategy_id,
                            mint = proposal.mint,
                            side = ?order.side,
                            fill_price = %fill.price,
                            "paper fill"
                        );
                        if order.side == OrderSide::Buy {
                            self.risk.note_entry(&order.mint, now).await;
                        }
                        let rec = persist_signal_rec(
                            &self.db,
                            proposal,
                            SignalDecision::Submitted,
                            None,
                            Some(&order_id),
                            now,
                        )
                        .await?;
                        self.bus.emit(WsEvent::Order(order_clone));
                        self.bus.emit(WsEvent::Fill(fill_clone));
                        self.bus.emit(WsEvent::Signal(rec));
                        Ok(SubmitOutcome::Filled { order, fill })
                    }
                    crate::trading::apply::ApplyOutcome::Rejected { order, reason } => {
                        let order_id = order.id.clone();
                        let order_clone = order.clone();
                        tx.commit().await?;
                        let rec = persist_signal_rec(
                            &self.db,
                            proposal,
                            SignalDecision::Rejected,
                            Some(reason.clone()),
                            Some(&order_id),
                            now,
                        )
                        .await?;
                        self.record_risk_event(
                            proposal,
                            "executor-rejection",
                            &reason,
                            Some(&order_id),
                        )
                        .await;
                        self.bus.emit(WsEvent::Signal(rec));
                        self.bus.emit(WsEvent::Order(order_clone));
                        Ok(SubmitOutcome::Rejected { order, reason })
                    }
                    crate::trading::apply::ApplyOutcome::Pending { order } => {
                        let order_id = order.id.clone();
                        let order_clone = order.clone();
                        tx.commit().await?;
                        let rec = persist_signal_rec(
                            &self.db,
                            proposal,
                            SignalDecision::Submitted,
                            Some("limit-order-open".into()),
                            Some(&order_id),
                            now,
                        )
                        .await?;
                        self.bus.emit(WsEvent::Order(order_clone));
                        self.bus.emit(WsEvent::Signal(rec));
                        Ok(SubmitOutcome::Pending { order })
                    }
                }
            }
            Err(e) => {
                // Rollback path — nothing persisted. The failed attempt is
                // recorded OUTSIDE this transaction so it survives.
                let _ = tx.rollback().await;
                let err_str = e.to_string();
                let _ = persist_signal_rec(
                    &self.db,
                    proposal,
                    SignalDecision::Rejected,
                    Some(format!("database-error: {err_str}")),
                    None,
                    now,
                )
                .await;
                self.record_risk_event(proposal, "database-error", &err_str, None)
                    .await;
                tracing::error!(
                    strategy = proposal.strategy_id,
                    error = %e,
                    "trade application failed, transaction rolled back"
                );
                Err(e)
            }
        }
    }

    /// Build the risk context from current account/position state (called
    /// inside the actor, so state is already consistent).
    async fn build_risk_context(
        &self,
        _proposal: &SignalProposal,
        snap: &MarketSnapshot,
    ) -> Result<RiskContext, AppError> {
        let account = repos::accounts::get_account(&self.db, &self.account_id).await?;
        let positions = repos::trading::list_positions(&self.db, &self.account_id).await?;
        let marks = crate::trading::portfolio::marks_from_snapshots(
            &self.cache.latest_all().await,
            &self.sol_mint,
        );
        let val = crate::trading::portfolio::equity(&account, &positions, &marks);
        let position_values = positions
            .iter()
            .filter(|p| p.qty_raw > 0)
            .map(|p| {
                let price = marks.price_of(&p.mint).unwrap_or(Decimal::ZERO);
                let human = crate::trading::models::math::raw_to_human(p.qty_raw, p.decimals());
                (p.mint.clone(), human * price)
            })
            .collect();
        let day_start = repos::accounts::day_start_equity_usd(&self.db, &self.account_id).await?;
        let peak = repos::accounts::peak_equity_usd(&self.db, &self.account_id)
            .await?
            .max(val.equity_usd);
        Ok(RiskContext {
            now: now_micros(),
            equity_usd: val.equity_usd,
            positions_open: positions.iter().filter(|p| p.qty_raw > 0).count(),
            position_values,
            day_start_equity: day_start,
            peak_equity: peak,
            snapshot: Some(snap.clone()),
            solusd: marks.solusd,
            executor_slippage_bps: self.cfg.paper_trading.slippage_bps,
            max_order_notional_usd: rust_decimal::Decimal::from(self.cfg.paper_trading.max_order_notional_usd),
        })
    }

    // -------- limit-order poll (marketable check only) --------
    async fn poll_limit_locked(
        &self,
        order_id: &str,
        fee_model: &FeeModel,
        slippage: i64,
    ) -> Result<Option<SubmitOutcome>, AppError> {
        let order = match repos::trading::get_order(&self.db, order_id).await {
            Ok(o) => o,
            Err(AppError::NotFound(_)) => return Ok(None),
            Err(e) => return Err(e),
        };
        if order.status != crate::domain::OrderStatus::Open {
            return Ok(None);
        }
        let Some(snap) = self.cache.latest_for(&order.mint).await else {
            return Ok(None);
        };
        let marketable = match (order.side, order.limit_price) {
            (OrderSide::Buy, Some(lp)) => snap.price <= lp,
            (OrderSide::Sell, Some(lp)) => snap.price >= lp,
            _ => false,
        };
        if !marketable {
            return Ok(None);
        }
        let solusd = self
            .cache
            .sol_price(&self.sol_mint)
            .await
            .unwrap_or_else(|| Decimal::from(150));
        let tx = self.db.transaction().await?;
        let ctx = FillContext {
            snapshot: snap,
            solusd,
            fee_model: *fee_model,
            slippage_bps: slippage,
        };
        match crate::trading::apply::fill_open_order(&tx, &order, &ctx).await {
            Ok(crate::trading::apply::ApplyOutcome::Filled { order, fill }) => {
                tx.commit().await?;
                self.bus.emit(WsEvent::Order(order.clone()));
                self.bus.emit(WsEvent::Fill(fill.clone()));
                tracing::info!(order = order.id, "limit order filled by watcher");
                Ok(Some(SubmitOutcome::Filled { order, fill }))
            }
            Ok(crate::trading::apply::ApplyOutcome::Rejected { order, reason }) => {
                tx.commit().await?;
                self.bus.emit(WsEvent::Order(order.clone()));
                Ok(Some(SubmitOutcome::Rejected { order, reason }))
            }
            Ok(crate::trading::apply::ApplyOutcome::Pending { order }) => {
                tx.commit().await?;
                Ok(Some(SubmitOutcome::Pending { order }))
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn record_risk_event(
        &self,
        proposal: &SignalProposal,
        rule: &str,
        detail: &str,
        order_id: Option<&str>,
    ) {
        let ev = crate::domain::RiskEvent {
            id: ids::new_id(),
            ts: now_micros(),
            kind: "order-rejected".into(),
            strategy_id: Some(proposal.strategy_id.clone()),
            mint: Some(proposal.mint.clone()),
            order_id: order_id.map(str::to_string),
            rule: rule.to_string(),
            detail: if detail.is_empty() { None } else { Some(detail.to_string()) },
        };
        if let Err(e) = repos::events::record_risk_event(&self.db, &ev).await {
            tracing::error!(error = %e, "failed to persist risk event");
        }
        self.bus.emit(WsEvent::Risk(ev));
    }
}

/// Persist a signal record (called with the executor's DB handle; the order
/// id is attached when one was created).
async fn persist_signal_rec(
    db: &Database,
    proposal: &SignalProposal,
    decision: SignalDecision,
    rejection_reason: Option<String>,
    order_id: Option<&str>,
    now: i64,
) -> Result<SignalRecord, AppError> {
    let rec = SignalRecord {
        id: ids::new_id(),
        strategy_id: proposal.strategy_id.clone(),
        strategy_version: proposal.strategy_version,
        mint: proposal.mint.clone(),
        direction: proposal.direction,
        score: proposal.score,
        reason: proposal.reason.clone(),
        proposed_size: proposal.proposed_size,
        decision,
        rejection_reason,
        data_observed_at: proposal.data_observed_at,
        generated_at: now,
        order_id: order_id.map(str::to_string),
    };
    repos::strategies::insert_signal(db, &rec).await?;
    Ok(rec)
}

// Private helper so `Position::decimals()` isn't needed publicly.
trait PositionDecimals {
    fn decimals(&self) -> i32;
}
impl PositionDecimals for crate::domain::Position {
    fn decimals(&self) -> i32 {
        9 // simulator convention; refined when real metadata is available
    }
}