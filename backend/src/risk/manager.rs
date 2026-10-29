//! Central risk manager.
//!
//! EVERY proposed simulated order passes through `evaluate` before the paper
//! engine may act on it. The manager is strategy-agnostic: it never learns
//! which strategy asked beyond the proposal, and strategies cannot override,
//! disable, or bypass its limits.
//!
//! Design notes:
//! * `evaluate` is a pure, deterministic function of its inputs — unit tests
//!   exercise every rule boundary without a database.
//! * Hot running state (per-mint entry cooldowns) lives in the manager;
//!   portfolio-level inputs (equity, exposure, drawdown, daily loss) are
//!   computed by the caller from the ledger and equity snapshots and passed
//!   in as `RiskContext`, so the manager stays independent of storage.
//! * Loss-limit / drawdown policies stop NEW entries only; they never force
//!   liquidations. Selling is always permitted (that lowers exposure), which
//!   is the documented policy for a loss limit reached while positions are
//!   still open.

use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use tokio::sync::Mutex;

use crate::config::RiskConfig;
use crate::domain::{Direction, MarketSnapshot, SignalProposal};
use crate::errors::AppError;
use crate::time;

/// Portfolio context assembled by the caller (the paper executor) from the
/// ledger and latest market observations.
#[derive(Debug, Clone)]
pub struct RiskContext {
    pub now: i64,
    /// Total equity in USD (cash + marked positions).
    pub equity_usd: Decimal,
    /// Number of currently open positions.
    pub positions_open: usize,
    /// Current USD value of each open position (mint -> value).
    pub position_values: HashMap<String, Decimal>,
    /// Equity at the start of today (None when there is no prior snapshot).
    pub day_start_equity: Option<Decimal>,
    /// Peak equity observed (USD), for drawdown measurement.
    pub peak_equity: Decimal,
    /// Latest validated snapshot for the proposal mint.
    pub snapshot: Option<MarketSnapshot>,
    /// Latest SOL/USD price (used for exposure math).
    pub solusd: Decimal,
    /// Slippage the executor would apply (bps) — must not exceed the cap.
    pub executor_slippage_bps: i64,
    /// Maximum notional of a single order (USD).
    pub max_order_notional_usd: Decimal,
}

/// Outcome of a risk evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskDecision {
    pub approved: bool,
    /// Rule that rejected the proposal, when applicable.
    pub rule: Option<String>,
    /// Human detail for dashboards and risk_event rows.
    pub detail: Option<String>,
}

impl RiskDecision {
    pub fn approve() -> RiskDecision {
        RiskDecision {
            approved: true,
            rule: None,
            detail: None,
        }
    }
    pub fn reject(rule: &str, detail: impl Into<String>) -> RiskDecision {
        RiskDecision {
            approved: false,
            rule: Some(rule.to_string()),
            detail: Some(detail.into()),
        }
    }
}

struct HotState {
    /// mint -> timestamp (micros) of the last approved entry (cooldown map).
    last_entry: HashMap<String, i64>,
}

/// The central risk manager (shared, `Arc`-wrapped).
pub struct RiskManager {
    cfg: RiskConfig,
    hot: Mutex<HotState>,
}

impl RiskManager {
    pub fn new(cfg: RiskConfig) -> RiskManager {
        RiskManager {
            cfg,
            hot: Mutex::new(HotState {
                last_entry: HashMap::new(),
            }),
        }
    }

    pub fn config(&self) -> &RiskConfig {
        &self.cfg
    }

    /// Register an approved entry for cooldown bookkeeping.
    pub async fn note_entry(&self, mint: &str, at: i64) {
        self.hot.lock().await.last_entry.insert(mint.to_string(), at);
    }

    /// Evaluate one proposal against every configured rule.
    /// Pure: does not mutate state except on internal cooldown read.
    pub async fn evaluate(
        &self,
        proposal: &SignalProposal,
        ctx: &RiskContext,
    ) -> Result<RiskDecision, AppError> {
        let cfg = &self.cfg;

        // ---- sanity ------------------------------------------------------
        if proposal.direction == Direction::Hold {
            return Ok(RiskDecision::reject(
                "hold-not-an-order",
                "hold signals never reach the order engine",
            ));
        }
        let Some(snap) = &ctx.snapshot else {
            return Ok(RiskDecision::reject(
                "no-price",
                "no valid market snapshot for this token",
            ));
        };

        // ---- freshness ----------------------------------------------------
        if time::age_ms(snap.observed_at) > cfg.max_price_age_ms as i64 {
            return Ok(RiskDecision::reject(
                "stale-price",
                format!(
                    "price observation is {} ms old (limit {} ms)",
                    time::age_ms(snap.observed_at),
                    cfg.max_price_age_ms
                ),
            ));
        }
        if snap.price <= Decimal::ZERO {
            return Ok(RiskDecision::reject(
                "invalid-price",
                "snapshot price is not positive; refusing to trade at it",
            ));
        }

        // ---- liquidity / volume -------------------------------------------
        if let Some(l) = snap.liquidity_usd {
            if l < cfg.minimum_liquidity_usd {
                return Ok(RiskDecision::reject(
                    "insufficient-liquidity",
                    format!(
                        "liquidity {l} is below the configured minimum {}",
                        cfg.minimum_liquidity_usd
                    ),
                ));
            }
        }
        if let Some(v) = snap.volume_24h_usd {
            if v < cfg.minimum_volume_24h_usd {
                return Ok(RiskDecision::reject(
                    "insufficient-volume",
                    format!(
                        "24h volume {v} is below the configured minimum {}",
                        cfg.minimum_volume_24h_usd
                    ),
                ));
            }
        }

        // ---- config-level execution-cost sanity -----------------------------
        if ctx.executor_slippage_bps > cfg.max_slippage_bps {
            return Ok(RiskDecision::reject(
                "slippage-cap",
                format!(
                    "executor slippage {} bps exceeds the configured cap {} bps",
                    ctx.executor_slippage_bps, cfg.max_slippage_bps
                ),
            ));
        }

        // ---- sizing / exposure / concentration (buy-side entries only) ----
        // Sells reduce exposure and are the recovery path when limits trigger;
        // they skip sizing/exposure/loss-limit/cooldown gates. The executor
        // still verifies the holding exists and caps the quantity.
        if proposal.direction == Direction::Buy {
            let size_usd = proposal
                .proposed_size
                .and_then(|c| Decimal::from(c).checked_div(Decimal::from(100))) // cents -> dollars
                .unwrap_or(Decimal::ZERO);
            if size_usd <= Decimal::ZERO {
                return Ok(RiskDecision::reject(
                    "zero-size",
                    "proposed size must be positive",
                ));
            }
            if size_usd > ctx.max_order_notional_usd {
                return Ok(RiskDecision::reject(
                    "max-order-notional",
                    format!(
                        "order notional {size_usd} exceeds cap {}",
                        ctx.max_order_notional_usd
                    ),
                ));
            }

            if ctx.equity_usd <= Decimal::ZERO {
                return Ok(RiskDecision::reject(
                    "no-equity",
                    "account equity is zero; no new entries allowed",
                ));
            }

            let current_pos_value =
                ctx.position_values.get(&proposal.mint).copied().unwrap_or(Decimal::ZERO);
            let projected_pos_value = current_pos_value + size_usd;
            let pos_fraction = projected_pos_value / ctx.equity_usd;
            if pos_fraction
                > Decimal::from_f64_retain(cfg.max_position_fraction).unwrap_or(Decimal::ONE)
            {
                return Ok(RiskDecision::reject(
                    "max-position-fraction",
                    format!(
                        "projected position {projected_pos_value} would be {pos_fraction:.2} of equity (limit {})",
                        cfg.max_position_fraction
                    ),
                ));
            }

            let held_mints = ctx.position_values.len();
            let projected_open =
                if current_pos_value > Decimal::ZERO { held_mints } else { held_mints + 1 };
            if projected_open > cfg.max_open_positions {
                return Ok(RiskDecision::reject(
                    "max-open-positions",
                    format!(
                        "would exceed the {} open position limit",
                        cfg.max_open_positions
                    ),
                ));
            }

            // ---- aggregate exposure ----
            let aggregate: Decimal = ctx.position_values.values().sum();
            let projected_exposure = (aggregate + size_usd) / ctx.equity_usd;
            if projected_exposure
                > Decimal::from_f64_retain(cfg.max_total_exposure).unwrap_or(Decimal::ONE)
            {
                return Ok(RiskDecision::reject(
                    "max-total-exposure",
                    format!(
                        "projected exposure {projected_exposure:.2} of equity (limit {})",
                        cfg.max_total_exposure
                    ),
                ));
            }

            // ---- loss limits (entries only) ----
            if let Some(ds) = ctx.day_start_equity {
                if ds > Decimal::ZERO {
                    let loss_frac = (ctx.equity_usd - ds) / ds;
                    if loss_frac
                        < -Decimal::from_f64_retain(cfg.max_daily_loss).unwrap_or(Decimal::ZERO)
                    {
                        return Ok(RiskDecision::reject(
                            "daily-loss-limit",
                            format!(
                                "daily loss {loss_frac:.2} exceeds the {} limit",
                                cfg.max_daily_loss
                            ),
                        ));
                    }
                }
            }

            if ctx.peak_equity > Decimal::ZERO {
                let drawdown = (ctx.peak_equity - ctx.equity_usd) / ctx.peak_equity;
                if drawdown > Decimal::from_f64_retain(cfg.max_drawdown).unwrap_or(Decimal::ZERO) {
                    return Ok(RiskDecision::reject(
                        "max-drawdown",
                        format!(
                            "drawdown {drawdown:.2} exceeds the {} limit",
                            cfg.max_drawdown
                        ),
                    ));
                }
            }

            // ---- per-token cooldown (entries only) ----
            let hot = self.hot.lock().await;
            if let Some(last) = hot.last_entry.get(&proposal.mint) {
                let elapsed_ms = (ctx.now - last) / 1_000;
                if elapsed_ms < cfg.per_token_cooldown_ms as i64 {
                    return Ok(RiskDecision::reject(
                        "token-cooldown",
                        format!(
                            "last entry {elapsed_ms} ms ago (cooldown {} ms)",
                            cfg.per_token_cooldown_ms
                        ),
                    ));
                }
            }
        }

        Ok(RiskDecision::approve())
    }
}

/// Convenience conversion for executor plumbing.
pub fn arc(cfg: RiskConfig) -> Arc<RiskManager> {
    Arc::new(RiskManager::new(cfg))
}