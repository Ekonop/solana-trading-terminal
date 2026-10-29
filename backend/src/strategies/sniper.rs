//! Sniper — new-pair / newly-observed token opportunities.
//!
//! This is a heuristic research model. It reacts to mints that are newly
//! observed in the feed with sufficient liquidity and accelerating activity.
//! It does NOT claim to detect a token "launch"; it only signals that the
//! data shows a young, active, liquid-enough pair by the configured rules.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::{Direction, SignalProposal};
use crate::errors::AppError;
use crate::strategies::config::*;
use crate::strategies::traits::{EvalContext, Strategy};

pub const ID: &str = "sniper";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SniperConfig {
    pub min_liquidity_usd: Decimal,
    pub min_volume_24h_usd: Decimal,
    /// Consecutive observations required before a new mint becomes eligible
    /// (filters one-tick blips).
    pub min_observations: usize,
    /// Minimum relative-volume spike of the latest bar vs history.
    pub min_rel_volume: f64,
    /// Minimum recent price move (fraction) as acceleration evidence.
    pub min_price_rise_pct: f64,
    /// Fraction of equity proposed per entry.
    pub size_fraction: f64,
    /// Required snapshot quality (0-100).
    pub required_quality: i32,
}

impl Default for SniperConfig {
    fn default() -> Self {
        SniperConfig {
            min_liquidity_usd: Decimal::from(50_000u64),
            min_volume_24h_usd: Decimal::from(5_000u64),
            min_observations: 3,
            min_rel_volume: 1.5,
            min_price_rise_pct: 0.02,
            size_fraction: 0.05,
            required_quality: 70,
        }
    }
}

#[derive(Debug)]
pub struct Sniper;

#[async_trait::async_trait]
impl Strategy for Sniper {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> i32 {
        1
    }
    fn display_name(&self) -> &'static str {
        "Sniper"
    }
    fn description(&self) -> &'static str {
        "New-pair / newly-observed token opportunities with sufficient liquidity and early activity evidence."
    }
    fn default_config(&self) -> serde_json::Value {
        serde_json::to_value(SniperConfig::default()).unwrap()
    }
    fn validate_config(&self, cfg: &serde_json::Value) -> Result<(), AppError> {
        let c: SniperConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("sniper config: {e}")))?;
        c.validate()
    }
    async fn evaluate(
        &self,
        cfg: &serde_json::Value,
        ctx: &EvalContext<'_>,
    ) -> Result<Vec<SignalProposal>, AppError> {
        let c: SniperConfig = serde_json::from_value(cfg.clone())
            .map_err(|e| AppError::Strategy(format!("sniper config: {e}")))?;
        c.validate()?;

        let mut out = Vec::new();
        for snap in ctx.snapshots {
            if snap.mint == ctx.sol_mint || snap.mint == "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" {
                continue; // never snipe the base currencies
            }
            if !is_fresh(snap, ctx.max_price_age_ms) {
                continue;
            }
            if snap.quality < c.required_quality {
                continue;
            }
            if let Some(l) = snap.liquidity_usd {
                if l < c.min_liquidity_usd {
                    continue;
                }
            }
            if let Some(v) = snap.volume_24h_usd {
                if v < c.min_volume_24h_usd {
                    continue;
                }
            }
            let hist = ctx.cache.history_for(&snap.mint, 40).await;
            if hist.len() < c.min_observations {
                continue;
            }
            let series = Series::from_snapshots(&hist);
            let Some(rv) = series.rel_volume(4) else { continue };
            let Some(ret) = series.return_over(3) else { continue };
            if rv >= c.min_rel_volume && ret >= c.min_price_rise_pct {
                out.push(SignalProposal {
                    strategy_id: ctx.strategy_id.to_string(),
                    strategy_version: ctx.strategy_version,
                    mint: snap.mint.clone(),
                    market_id: snap.pair_id.clone(),
                    direction: Direction::Buy,
                    score: Some(dec(rv * 100.0)),
                    reason: format!(
                        "new-pair candidate: {} tick history, rel-volume {rv:.2}, recent move {:.2}%",
                        hist.len(),
                        ret * 100.0
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

impl SniperConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.min_observations < 2 {
            return Err(AppError::Validation { field: "min_observations".into(), message: "must be >= 2".into() });
        }
        if !(0.0..=0.5).contains(&self.size_fraction) {
            return Err(AppError::Validation { field: "size_fraction".into(), message: "must be in [0, 0.5]".into() });
        }
        if self.min_liquidity_usd < Decimal::ZERO || self.min_volume_24h_usd < Decimal::ZERO {
            return Err(AppError::Validation { field: "liquidity".into(), message: "must be >= 0".into() });
        }
        Ok(())
    }
}