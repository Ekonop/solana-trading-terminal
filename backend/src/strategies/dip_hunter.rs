//! Dip Hunter — pullbacks within an established uptrend.
//!
//! Distinguishes a pullback from a downtrend by requiring a prior
//! directional trend, a measured pullback from a rolling high, and a recovery
//! confirmation. An invalidation level (below the recent low) cancels the
//! thesis.

use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::domain::{Direction, SignalProposal};
use crate::errors::AppError;
use crate::strategies::config::*;
use crate::strategies::traits::{EvalContext, Strategy};

pub const ID: &str = "dip_hunter";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct DipHunterConfig {
    /// Bars used to establish the prior trend.
    pub trend_window: usize,
    /// Minimum uptrend return over `trend_window` to qualify as a trend.
    pub trend_min_return: f64,
    /// Pullback depth from local high (fraction) to qualify as a dip.
    pub pullback_max_pct: f64,
    /// Recovery requirement: price must be this fraction above the dip low.
    pub recovery_confirmation: f64,
    pub min_history: usize,
    /// Drop below the recent low by this fraction invalidates (sell).
    pub invalidation_pct: f64,
    pub size_fraction: f64,
}

impl Default for DipHunterConfig {
    fn default() -> Self {
        DipHunterConfig {
            trend_window: 16,
            trend_min_return: 0.08,
            pullback_max_pct: 0.10,
            recovery_confirmation: 0.01,
            min_history: 24,
            invalidation_pct: 0.03,
            size_fraction: 0.06,
        }
    }
}

impl DipHunterConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.trend_window < 8 || self.min_history < self.trend_window + 4 {
            return Err(AppError::Validation {
                field: "min_history".into(),
                message: "must be at least trend-window + 4".into(),
            });
        }
        if self.pullback_max_pct <= 0.0 || self.pullback_max_pct >= 0.5 {
            return Err(AppError::Validation {
                field: "pullback_max_pct".into(),
                message: "must be in (0, 0.5)".into(),
            });
        }
        if !(0.0..=1.0).contains(&self.size_fraction) {
            return Err(AppError::Validation {
                field: "size_fraction".into(),
                message: "must be in [0, 1]".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DipHunter;

#[async_trait::async_trait]
impl Strategy for DipHunter {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> i32 {
        1
    }
    fn display_name(&self) -> &'static str {
        "Dip Hunter"
    }
    fn description(&self) -> &'static str {
        "Pullbacks within an established uptrend, with recovery confirmation and a defined invalidation level."
    }
    fn default_config(&self) -> serde_json::Value {
        serde_json::to_value(DipHunterConfig::default()).unwrap()
    }
    fn validate_config(&self, cfg: &serde_json::Value) -> Result<(), AppError> {
        let c: DipHunterConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("dip_hunter config: {e}")))?;
        c.validate()
    }
    async fn evaluate(
        &self,
        cfg: &serde_json::Value,
        ctx: &EvalContext<'_>,
    ) -> Result<Vec<SignalProposal>, AppError> {
        let c: DipHunterConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("dip_hunter config: {e}")))?;
        c.validate()?;

        let mut out = Vec::new();
        for snap in ctx.snapshots {
            if snap.mint == ctx.sol_mint {
                continue;
            }
            if !is_fresh(snap, ctx.max_price_age_ms) {
                continue;
            }
            let hist = ctx.cache.history_for(&snap.mint, 80).await;
            let series = Series::from_snapshots(&hist);
            if series.len() < c.min_history {
                continue;
            }
            let price = series.last_price().unwrap_or_default();
            if price <= 0.0 {
                continue;
            }
            let trend_ret = series.return_over(c.trend_window).unwrap_or(0.0);
            let high = series.local_high(c.trend_window).unwrap_or(price);

            let held = ctx
                .positions
                .iter()
                .find(|p| p.mint == snap.mint && p.qty_raw > 0);

            if trend_ret < c.trend_min_return {
                // No established uptrend — a decline here is a downtrend, not
                // a dip. Do not blindly buy declines.
                if held.is_some_and(|p| {
                    let entry = p.avg_entry_price.to_f64().unwrap_or_default();
                    entry > 0.0 && price / entry <= 1.0 - c.invalidation_pct
                }) {
                    out.push(invalidate(ctx, snap, price, "trend broke; invalidation"));
                }
                continue;
            }

            let drawdown = (high - price) / high;
            if drawdown < 0.0 {
                continue; // at a new high — not a dip
            }
            // Recent low for invalidation.
            let recent_low = series
                .prices
                .iter()
                .rev()
                .take(10)
                .copied()
                .fold(f64::INFINITY, f64::min);
            let below_low = (recent_low - price) / recent_low;

            if let Some(_pos) = held {
                if below_low >= c.invalidation_pct {
                    out.push(invalidate(ctx, snap, price, "price broke the recent low"));
                }
                continue; // no averaging down
            }

            // A dip qualifies when it is a shallow-to-moderate pullback and
            // price has started recovering from the dip low.
            let dip_low = series.prices.iter().rev().take(10).copied().fold(f64::INFINITY, f64::min);
            let recovery = (price - dip_low) / dip_low;
            if drawdown <= c.pullback_max_pct && recovery >= c.recovery_confirmation {
                out.push(SignalProposal {
                    strategy_id: ctx.strategy_id.to_string(),
                    strategy_version: ctx.strategy_version,
                    mint: snap.mint.clone(),
                    market_id: snap.pair_id.clone(),
                    direction: Direction::Buy,
                    score: Some(dec(100.0 - drawdown * 1000.0)),
                    reason: format!(
                        "dip within uptrend: drawdown {drawdown:.2} from high, recovery {recovery:.2}"
                    ),
                    proposed_size: Some(size_usd_cents(ctx.equity_usd, c.size_fraction)),
                    ref_price: snap.price,
                    data_observed_at: snap.observed_at,
                    snapshot_id: Some(snap.id.clone()),
                });
            }
        }
        Ok(out)
    }
}

fn invalidate(
    ctx: &EvalContext<'_>,
    snap: &crate::domain::MarketSnapshot,
    price: f64,
    reason: &str,
) -> SignalProposal {
    SignalProposal {
        strategy_id: ctx.strategy_id.to_string(),
        strategy_version: ctx.strategy_version,
        mint: snap.mint.clone(),
        market_id: snap.pair_id.clone(),
        direction: Direction::Sell,
        score: Some(dec(0.0)),
        reason: format!("{reason}: price {price:.8}"),
        proposed_size: Some(crate::strategies::config::size_usd_cents(
            ctx.equity_usd,
            ctx.positions
                .iter()
                .find(|p| p.mint == snap.mint)
                .map(|_| 1.0)
                .unwrap_or(0.0),
        )),
        ref_price: snap.price,
        data_observed_at: snap.observed_at,
        snapshot_id: Some(snap.id.clone()),
    }
}