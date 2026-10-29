//! Momentum — price/volume trend detection with confirmation and invalidation.

use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::domain::{Direction, SignalProposal};
use crate::errors::AppError;
use crate::strategies::config::*;
use crate::strategies::traits::{EvalContext, Strategy};

pub const ID: &str = "momentum";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct MomentumConfig {
    pub short_window: usize,
    pub long_window: usize,
    pub momentum_threshold: f64,
    pub rel_volume_threshold: f64,
    /// Bars the trend must persist before entry.
    pub entry_confirmation_period: usize,
    /// Exit signal when price drops this much from its rolling high.
    pub exit_drop_pct: f64,
    pub take_profit_pct: f64,
    pub size_fraction: f64,
    pub min_history: usize,
}

impl Default for MomentumConfig {
    fn default() -> Self {
        MomentumConfig {
            short_window: 4,
            long_window: 12,
            momentum_threshold: 0.02,
            rel_volume_threshold: 1.2,
            entry_confirmation_period: 2,
            exit_drop_pct: 0.05,
            take_profit_pct: 0.10,
            size_fraction: 0.08,
            min_history: 16,
        }
    }
}

impl MomentumConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.short_window < 2 || self.long_window <= self.short_window {
            return Err(AppError::Validation {
                field: "windows".into(),
                message: "short-window must be >= 2 and < long-window".into(),
            });
        }
        if !(0.0..=1.0).contains(&self.size_fraction) {
            return Err(AppError::Validation {
                field: "size_fraction".into(),
                message: "must be in [0, 1]".into(),
            });
        }
        if self.momentum_threshold <= 0.0 {
            return Err(AppError::Validation {
                field: "momentum_threshold".into(),
                message: "must be > 0".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Momentum;

#[async_trait::async_trait]
impl Strategy for Momentum {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> i32 {
        1
    }
    fn display_name(&self) -> &'static str {
        "Momentum"
    }
    fn description(&self) -> &'static str {
        "Trend detection via windowed returns, moving-average relationships and relative volume."
    }
    fn default_config(&self) -> serde_json::Value {
        serde_json::to_value(MomentumConfig::default()).unwrap()
    }
    fn validate_config(&self, cfg: &serde_json::Value) -> Result<(), AppError> {
        let c: MomentumConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("momentum config: {e}")))?;
        c.validate()
    }
    async fn evaluate(
        &self,
        cfg: &serde_json::Value,
        ctx: &EvalContext<'_>,
    ) -> Result<Vec<SignalProposal>, AppError> {
        let c: MomentumConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("momentum config: {e}")))?;
        c.validate()?;

        let mut out = Vec::new();
        for snap in ctx.snapshots {
            if snap.mint == ctx.sol_mint {
                continue;
            }
            if !is_fresh(snap, ctx.max_price_age_ms) {
                continue;
            }
            // Existing holdings get exit/invalidation checks first.
            let held = ctx
                .positions
                .iter()
                .find(|p| p.mint == snap.mint && p.qty_raw > 0);
            if let Some(pos) = held {
                let entry = pos.avg_entry_price.to_f64().unwrap_or_default();
                let cur = snap.price.to_f64().unwrap_or_default();
                if entry > 0.0 && cur > 0.0 {
                    let hist = ctx.cache.history_for(&snap.mint, 40).await;
                    let series = Series::from_snapshots(&hist);
                    let high = series.local_high(10).unwrap_or(cur);
                    let drop_from_high = (high - cur) / high;
                    let gain = (cur - entry) / entry;
                    if drop_from_high >= c.exit_drop_pct {
                        out.push(SignalProposal {
                            strategy_id: ctx.strategy_id.to_string(),
                            strategy_version: ctx.strategy_version,
                            mint: snap.mint.clone(),
                            market_id: snap.pair_id.clone(),
                            direction: Direction::Sell,
                            score: Some(dec(drop_from_high * 100.0)),
                            reason: format!(
                                "momentum invalidation: price {drop_from_high:.2} below 10-bar high"
                            ),
                            proposed_size: Some(size_usd_cents(ctx.equity_usd, c.size_fraction)),
                            ref_price: snap.price,
                            data_observed_at: snap.observed_at,
                            snapshot_id: Some(snap.id.clone()),
                        });
                        continue;
                    }
                    if gain >= c.take_profit_pct && c.take_profit_pct > 0.0 {
                        out.push(SignalProposal {
                            strategy_id: ctx.strategy_id.to_string(),
                            strategy_version: ctx.strategy_version,
                            mint: snap.mint.clone(),
                            market_id: snap.pair_id.clone(),
                            direction: Direction::Sell,
                            score: Some(dec(gain * 100.0)),
                            reason: format!("momentum take-profit hit: +{gain:.2}"),
                            proposed_size: Some(size_usd_cents(ctx.equity_usd, c.size_fraction)),
                            ref_price: snap.price,
                            data_observed_at: snap.observed_at,
                            snapshot_id: Some(snap.id.clone()),
                        });
                        continue;
                    }
                }
                if !held_exitable(&ctx.positions, &snap.mint) {
                    continue;
                }
            } else {
                // Entry: require the trend plus confirmation.
                let hist = ctx.cache.history_for(&snap.mint, 60).await;
                let series = Series::from_snapshots(&hist);
                if series.len() < c.min_history {
                    continue;
                }
                let Some(short_ret) = series.return_over(c.short_window) else { continue };
                let Some(long_ret) = series.return_over(c.long_window) else { continue };
                let Some(sma_s) = series.sma(c.short_window) else { continue };
                let Some(sma_l) = series.sma(c.long_window) else { continue };
                let Some(rv) = series.rel_volume(c.short_window) else { continue };
                let price = series.last_price().unwrap_or_default();

                let trend_ok = short_ret >= c.momentum_threshold
                    && long_ret > 0.0
                    && sma_s > sma_l
                    && rv >= c.rel_volume_threshold;
                let confirmed = series
                    .prices
                    .iter()
                    .rev()
                    .take(c.entry_confirmation_period)
                    .all(|p| *p >= price * (1.0 - c.momentum_threshold));

                if trend_ok && confirmed && price > 0.0 {
                    out.push(SignalProposal {
                        strategy_id: ctx.strategy_id.to_string(),
                        strategy_version: ctx.strategy_version,
                        mint: snap.mint.clone(),
                        market_id: snap.pair_id.clone(),
                        direction: Direction::Buy,
                        score: Some(dec(short_ret * 1000.0)),
                        reason: format!(
                            "momentum: {}-bar return {short_ret:.2}, sma{s}/{sma_l:.4}, rel-vol {rv:.2}",
                            c.short_window, s = c.short_window
                        ),
                        proposed_size: Some(size_usd_cents(ctx.equity_usd, c.size_fraction)),
                        ref_price: snap.price,
                        data_observed_at: snap.observed_at,
                        snapshot_id: Some(snap.id.clone()),
                    });
                }
            }
        }
        Ok(out)
    }
}

/// Helper: true when the positions list has an open position for the mint.
fn held_exitable(positions: &[crate::domain::Position], mint: &str) -> bool {
    positions
        .iter()
        .any(|p| p.mint == mint && p.qty_raw > 0 && p.status == "open")
}