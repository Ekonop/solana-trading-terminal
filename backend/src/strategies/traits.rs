//! Strategy trait and shared evaluation context.

use rust_decimal::Decimal;

use crate::domain::{Position, SignalProposal};
use crate::errors::AppError;
use crate::market_data::cache::MarketCache;

/// Everything a strategy may observe during one evaluation pass.
#[derive(Debug, Clone)]
pub struct EvalContext<'a> {
    pub strategy_id: &'a str,
    pub strategy_version: i32,
    pub now: i64,
    pub sol_mint: &'a str,
    pub max_price_age_ms: u64,
    pub account_cash_lamports: i64,
    pub equity_usd: Decimal,
    pub positions: &'a [Position],
    /// Latest validated snapshot per mint (live view).
    pub snapshots: &'a [crate::domain::MarketSnapshot],
    /// History rings for windowed analysis.
    pub cache: &'a MarketCache,
}

/// A strategy is a pure function from (config, context) to proposals.
#[async_trait::async_trait]
pub trait Strategy: Send + Sync {
    /// Stable identifier (also the DB `strategies.id`).
    fn id(&self) -> &'static str;
    fn version(&self) -> i32;
    fn display_name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_config(&self) -> serde_json::Value;
    fn validate_config(&self, cfg: &serde_json::Value) -> Result<(), AppError>;
    /// Evaluate one batch of observations. Returns zero or more proposals.
    /// Must be deterministic for the same (config, context).
    async fn evaluate(&self, cfg: &serde_json::Value, ctx: &EvalContext<'_>)
        -> Result<Vec<SignalProposal>, AppError>;
}