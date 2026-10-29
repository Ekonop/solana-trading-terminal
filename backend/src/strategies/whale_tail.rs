//! Whale Tail — unusually large public activity on a token, using volume data
//! as the only observable the current providers expose.
//!
//! IMPORTANT LIMITATION (documented, not hidden): the providers in this build
//! (demo generator, DexScreener) do NOT expose on-chain transfer or swap
//! events per wallet. "Whale" activity is therefore proxied by statistically
//! unusual 24h-volume spikes. A volume spike is NOT proof of a large wallet
//! movement — transfers, liquidity operations and swaps are not
//! interchangeable, and the strategy's reason text says so. When the data
//! cannot support an interpretation we emit nothing and the dashboard shows
//! the unavailable-data state rather than invented whale events.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::{Direction, SignalProposal};
use crate::errors::AppError;
use crate::strategies::config::*;
use crate::strategies::traits::{EvalContext, Strategy};

pub const ID: &str = "whale_tail";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct WhaleTailConfig {
    /// Minimum volume spike (multiples of trailing average) considered.
    pub min_spike_multiple: f64,
    /// Minimum number of trailing bars to form the baseline.
    pub baseline_window: usize,
    /// Velocity requirement after the spike (subsequent price move fraction).
    pub follow_through_pct: f64,
    pub size_fraction: f64,
    pub min_history: usize,
}

impl Default for WhaleTailConfig {
    fn default() -> Self {
        WhaleTailConfig {
            min_spike_multiple: 3.0,
            baseline_window: 8,
            follow_through_pct: 0.01,
            size_fraction: 0.04,
            min_history: 20,
        }
    }
}

impl WhaleTailConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.min_spike_multiple < 1.5 {
            return Err(AppError::Validation {
                field: "min_spike_multiple".into(),
                message: "must be >= 1.5 to mean something".into(),
            });
        }
        if self.baseline_window < 3 {
            return Err(AppError::Validation {
                field: "baseline_window".into(),
                message: "must be >= 3".into(),
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
pub struct WhaleTail;

#[async_trait::async_trait]
impl Strategy for WhaleTail {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> i32 {
        1
    }
    fn display_name(&self) -> &'static str {
        "Whale Tail"
    }
    fn description(&self) -> &'static str {
        "Flags statistically unusual volume activity as a proxy for large public moves (proxy only — see docs)."
    }
    fn default_config(&self) -> serde_json::Value {
        serde_json::to_value(WhaleTailConfig::default()).unwrap()
    }
    fn validate_config(&self, cfg: &serde_json::Value) -> Result<(), AppError> {
        let c: WhaleTailConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("whale_tail config: {e}")))?;
        c.validate()
    }
    async fn evaluate(
        &self,
        cfg: &serde_json::Value,
        ctx: &EvalContext<'_>,
    ) -> Result<Vec<SignalProposal>, AppError> {
        let c: WhaleTailConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("whale_tail config: {e}")))?;
        c.validate()?;

        let mut out = Vec::new();
        for snap in ctx.snapshots {
            if snap.mint == ctx.sol_mint {
                continue;
            }
            if !is_fresh(snap, ctx.max_price_age_ms) {
                continue;
            }
            if snap.volume_24h_usd.unwrap_or(Decimal::ZERO) <= Decimal::ZERO {
                // No volume data → the whale proxy is unavailable, not zero.
                continue;
            }
            let hist = ctx.cache.history_for(&snap.mint, 60).await;
            let series = Series::from_snapshots(&hist);
            if series.len() < c.min_history {
                continue;
            }
            let Some(spike) = series.rel_volume(c.baseline_window) else { continue };
            if spike < c.min_spike_multiple {
                continue;
            }
            let Some(follow) = series.return_over(2) else { continue };
            if follow < c.follow_through_pct {
                // Spike without follow-through — no signal, don't chase.
                continue;
            }
            out.push(SignalProposal {
                strategy_id: ctx.strategy_id.to_string(),
                strategy_version: ctx.strategy_version,
                mint: snap.mint.clone(),
                market_id: snap.pair_id.clone(),
                direction: Direction::Buy,
                score: Some(dec(spike * 100.0)),
                reason: format!(
                    "unusual volume {spike:.1}x trailing average with {:.2}% follow-through \
                     (proxy for large public activity; not direct wallet evidence)",
                    follow * 100.0
                ),
                proposed_size: Some(size_usd_cents(ctx.equity_usd, c.size_fraction)),
                ref_price: snap.price,
                data_observed_at: snap.observed_at,
                snapshot_id: Some(snap.id.clone()),
            });
        }
        Ok(out)
    }
}