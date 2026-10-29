//! Core domain models shared across repositories, services, and the API.
//! These are plain Rust structs — no pglite row types leak past the
//! repository boundary.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub use crate::db::numeric::LAMPORTS_PER_SOL;

// ---------------------------------------------------------------------------
// Accounts & portfolio
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Account {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub initial_balance_lamports: i64,
    pub cash_balance_lamports: i64,
    pub realized_pnl_lamports: i64,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyState {
    Disabled,
    Ready,
    /// Evaluating but not submitting orders (read-only observation).
    Shadow,
    PaperTrading,
    Paused,
    Faulted,
}

impl StrategyState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StrategyState::Disabled => "disabled",
            StrategyState::Ready => "ready",
            StrategyState::Shadow => "shadow",
            StrategyState::PaperTrading => "paper_trading",
            StrategyState::Paused => "paused",
            StrategyState::Faulted => "faulted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StrategyModel {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub status: StrategyState,
    pub enabled: bool,
    pub config_json: String,
    pub config_version: i32,
    pub last_eval_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Observations come from a live (or demo) provider loop.
    Live,
    /// Observations are replayed from persisted history.
    Replay,
    /// Deterministic demo mode.
    Demo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StrategyRun {
    pub id: String,
    pub strategy_id: String,
    pub status: String,
    pub mode: RunMode,
    pub config_json: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Market data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MarketSnapshot {
    pub id: String,
    pub mint: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub pair_id: Option<String>,
    pub price: Decimal,
    pub liquidity_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub source: String,
    pub is_simulated: bool,
    pub observed_at: i64,
    pub received_at: i64,
    pub quality: i32,
}

impl MarketSnapshot {
    /// True when the observation is older than `max_age_ms`.
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        crate::time::age_ms(self.observed_at) > max_age_ms as i64
    }

    /// Human quality label derived from the quality score and recency.
    pub fn quality_label(&self, max_age_ms: u64) -> &'static str {
        if self.is_stale(max_age_ms) {
            "stale"
        } else if self.quality >= 80 {
            "good"
        } else if self.quality >= 50 {
            "degraded"
        } else {
            "poor"
        }
    }
}

// ---------------------------------------------------------------------------
// Signals
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalDecision {
    Submitted,
    Rejected,
}

/// What a strategy wants to do. Produced by strategies, consumed by the risk
/// manager and the paper executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SignalProposal {
    pub strategy_id: String,
    pub strategy_version: i32,
    pub mint: String,
    pub market_id: Option<String>,
    pub direction: Direction,
    pub score: Option<Decimal>,
    pub reason: String,
    /// Proposed size in raw base units (for buy) — validated by risk/exec.
    pub proposed_size: Option<i64>,
    /// Reference price used to size the order (USD per token).
    pub ref_price: Decimal,
    pub data_observed_at: i64,
    pub snapshot_id: Option<String>,
}

/// Persisted signal record (the full audit trail).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SignalRecord {
    pub id: String,
    pub strategy_id: String,
    pub strategy_version: i32,
    pub mint: String,
    pub direction: Direction,
    pub score: Option<Decimal>,
    pub reason: String,
    pub proposed_size: Option<i64>,
    pub decision: SignalDecision,
    pub rejection_reason: Option<String>,
    pub data_observed_at: i64,
    pub generated_at: i64,
    pub order_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Orders / fills / positions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Open,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PaperOrder {
    pub id: String,
    pub idempotency_key: String,
    pub account_id: String,
    pub strategy_id: Option<String>,
    pub mint: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub qty_raw: i64,
    pub limit_price: Option<Decimal>,
    pub status: OrderStatus,
    pub created_at: i64,
    pub filled_at: Option<i64>,
    pub cancel_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Fill {
    pub id: String,
    pub order_id: String,
    pub qty_raw: i64,
    pub price: Decimal,
    pub slippage_bps: Decimal,
    pub fee_lamports: i64,
    pub fee_usd: Decimal,
    pub snapshot_ref: Option<String>,
    pub filled_at: i64,
    pub is_paper: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Position {
    pub account_id: String,
    pub mint: String,
    pub qty_raw: i64,
    pub avg_entry_price: Decimal,
    pub status: String,
    pub opened_at: i64,
    pub updated_at: i64,
}

// ---------------------------------------------------------------------------
// Ledger / equity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerKind {
    InitialDeposit,
    Buy,
    Sell,
    Fee,
    RealizedPnl,
    Adjustment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LedgerEntry {
    pub id: String,
    pub account_id: String,
    pub ts: i64,
    pub kind: LedgerKind,
    /// Signed lambda movement in lamports.
    pub amount_lamports: i64,
    pub balance_after: i64,
    pub ref_id: Option<String>,
    pub correlation_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquitySnapshot {
    pub id: String,
    pub account_id: String,
    pub ts: i64,
    pub cash_lamports: i64,
    pub positions_value_usd: Decimal,
    pub equity_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub realized_pnl_lamports: i64,
    pub exposure_pct: Decimal,
}

// ---------------------------------------------------------------------------
// Risk & system events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RiskEvent {
    pub id: String,
    pub ts: i64,
    pub kind: String,
    pub strategy_id: Option<String>,
    pub mint: Option<String>,
    pub order_id: Option<String>,
    pub rule: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemLevel {
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SystemEvent {
    pub id: String,
    pub ts: i64,
    pub level: SystemLevel,
    pub subsystem: String,
    pub message: String,
    pub correlation_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Backtests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BacktestStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BacktestRun {
    pub id: String,
    pub strategy_id: String,
    pub strategy_version: i32,
    pub config_json: String,
    pub data_source: String,
    pub interval_start: i64,
    pub interval_end: i64,
    pub fee_model: String,
    pub slippage_bps: Decimal,
    pub status: BacktestStatus,
    pub metrics_json: Option<String>,
    pub net_return_pct: Option<Decimal>,
    pub max_drawdown_pct: Option<Decimal>,
    pub win_rate_pct: Option<Decimal>,
    pub trade_count: Option<i64>,
    pub profit_factor: Option<Decimal>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
}

/// Backtest metrics, stored as `metrics_json` (typed on the way out).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct BacktestMetrics {
    pub start_equity_usd: Decimal,
    pub end_equity_usd: Decimal,
    pub net_return_pct: Decimal,
    pub realized_pnl_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub max_drawdown_pct: Decimal,
    pub win_rate_pct: Option<Decimal>,
    pub trade_count: i64,
    pub wins: i64,
    pub losses: i64,
    pub avg_win_usd: Option<Decimal>,
    pub avg_loss_usd: Option<Decimal>,
    pub profit_factor: Option<Decimal>,
    pub fee_total_usd: Decimal,
    pub slippage_total_usd: Decimal,
    pub time_in_market_pct: Option<Decimal>,
    pub risk_rejections: i64,
    pub signals: i64,
    pub data_gap_percent: Decimal,
    pub qualified: bool,
    pub qualification_note: Option<String>,
}