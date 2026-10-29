//! Scalper — short-horizon opportunities in liquid markets.
//!
//! The scalper refuses to run when the observation cadence is too coarse or
//! when the expected edge after modeled fees + slippage is not positive. It
//! never fabricates a bid/ask spread: the configured `spread_proxy_bps` is an
//! explicit estimate labelled as a proxy, and the reason text says so.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::{Direction, SignalProposal};
use crate::errors::AppError;
use crate::strategies::config::*;
use crate::strategies::traits::{EvalContext, Strategy};

pub const ID: &str = "scalper";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ScalperConfig {
    pub window: usize,
    pub min_history: usize,
    /// Minimum expected move (bps) over `window` bars.
    pub min_edge_bps: i64,
    /// Estimated round-trip spread proxy (bps) — an explicit estimate.
    pub spread_proxy_bps: i64,
    /// Fee+slippage assumption used for the edge check (bps, one-way).
    pub cost_bps_per_trade: i64,
    /// Max cadence (ms) between observations; coarser data disables the
    /// strategy for that mint.
    pub max_interval_ms: i64,
    /// Max volatility (per-bar stdev of log returns) — very choppy markets
    /// are excluded.
    pub volatility_ceiling: f64,
    pub size_fraction: f64,
    pub min_liquidity_usd: Decimal,
}

impl Default for ScalperConfig {
    fn default() -> Self {
        ScalperConfig {
            window: 3,
            min_history: 16,
            min_edge_bps: 4,
            spread_proxy_bps: 30,
            cost_bps_per_trade: 75, // 25 fee + 50 slippage, one-way
            max_interval_ms: 30_000,
            volatility_ceiling: 0.05,
            size_fraction: 0.05,
            min_liquidity_usd: Decimal::from(100_000u64),
        }
    }
}

impl ScalperConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.window < 2 {
            return Err(AppError::Validation {
                field: "window".into(),
                message: "must be >= 2".into(),
            });
        }
        if self.cost_bps_per_trade < 0 || self.spread_proxy_bps < 0 {
            return Err(AppError::Validation {
                field: "costs".into(),
                message: "costs must be >= 0".into(),
            });
        }
        if self.max_interval_ms <= 0 {
            return Err(AppError::Validation {
                field: "max_interval_ms".into(),
                message: "must be > 0".into(),
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
pub struct Scalper;

#[async_trait::async_trait]
impl Strategy for Scalper {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> i32 {
        1
    }
    fn display_name(&self) -> &'static str {
        "Scalper"
    }
    fn description(&self) -> &'static str {
        "Short-horizon entries in liquid, fast-fed markets; disabled when costs exceed modeled edge."
    }
    fn default_config(&self) -> serde_json::Value {
        serde_json::to_value(ScalperConfig::default()).unwrap()
    }
    fn validate_config(&self, cfg: &serde_json::Value) -> Result<(), AppError> {
        let c: ScalperConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("scalper config: {e}")))?;
        c.validate()
    }
    async fn evaluate(
        &self,
        cfg: &serde_json::Value,
        ctx: &EvalContext<'_>,
    ) -> Result<Vec<SignalProposal>, AppError> {
        let c: ScalperConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("scalper config: {e}")))?;
        c.validate()?;

        let mut out = Vec::new();
        for snap in ctx.snapshots {
            if snap.mint == ctx.sol_mint {
                continue;
            }
            if !is_fresh(snap, ctx.max_price_age_ms) {
                continue;
            }
            if snap.liquidity_usd.unwrap_or(Decimal::ZERO) < c.min_liquidity_usd {
                continue; // scalping illiquid books is fantasy
            }
            let hist = ctx.cache.history_for(&snap.mint, 60).await;
            let series = Series::from_snapshots(&hist);
            if series.len() < c.min_history {
                continue;
            }
            // Cadence gate: if observations arrive slower than the config
            // allows, the strategy cannot justify short-horizon entries.
            if let Some(interval) = series.median_interval_ms() {
                if interval > c.max_interval_ms {
                    continue;
                }
            }
            // Volatility gate.
            if let Some(vol) = series.volatility(8) {
                if vol > c.volatility_ceiling {
                    continue;
                }
            }
            let Some(move_bps) = series
                .return_over(c.window)
                .map(|r| (r * 10_000.0).round() as i64)
            else {
                continue;
            };
            let round_trip_cost = c.cost_bps_per_trade
                + c.spread_proxy_bps.div_euclid(2); // half-spread each way (proxy)

            let held = ctx
                .positions
                .iter()
                .find(|p| p.mint == snap.mint && p.qty_raw > 0);
            if let Some(pos) = held {
                // Flush the dust when the short-term move turns negative.
                let entry = pos.avg_entry_price.to_f64().unwrap_or_default();
                let cur = snap.price.to_f64().unwrap_or_default();
                let gain_bps = if entry > 0.0 {
                    ((cur - entry) / entry * 10_000.0).round() as i64
                } else {
                    0
                };
                if gain_bps < round_trip_cost {
                    out.push(SignalProposal {
                        strategy_id: ctx.strategy_id.to_string(),
                        strategy_version: ctx.strategy_version,
                        mint: snap.mint.clone(),
                        market_id: snap.pair_id.clone(),
                        direction: Direction::Sell,
                        score: Some(dec((gain_bps as f64) / 100.0)),
                        reason: format!(
                            "scalper exit: gain {gain_bps} bps < round-trip cost {round_trip_cost} bps"
                        ),
                        proposed_size: Some(size_usd_cents(ctx.equity_usd, c.size_fraction)),
                        ref_price: snap.price,
                        data_observed_at: snap.observed_at,
                        snapshot_id: Some(snap.id.clone()),
                    });
                }
                continue;
            }

            // Entry requires positive expected edge net of modeled costs and
            // the spread proxy. Never claim a bid/ask spread from last price.
            if move_bps >= c.min_edge_bps && move_bps > round_trip_cost {
                out.push(SignalProposal {
                    strategy_id: ctx.strategy_id.to_string(),
                    strategy_version: ctx.strategy_version,
                    mint: snap.mint.clone(),
                    market_id: snap.pair_id.clone(),
                    direction: Direction::Buy,
                    score: Some(dec((move_bps - round_trip_cost) as f64 / 100.0)),
                    reason: format!(
                        "scalp: {move_bps} bps window move vs ~{round_trip_cost} bps modeled \
                         costs (spread proxy {})",
                        c.spread_proxy_bps
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