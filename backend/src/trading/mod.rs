//! Paper-trading engine: deterministic, auditable, simulation-only execution.
//!
//! Every order is applied inside a single pglite transaction: order row,
//! fill row, cash movement, fee, position upsert, and ledger entries either
//! all commit together or all roll back together. Idempotency keys make
//! duplicate submissions replay-safe.

pub mod apply;
pub mod executor;
pub mod fees;
pub mod models;
pub mod portfolio;

pub use apply::{insert_and_fill, ApplyOutcome, FillContext};
pub use executor::{Executor, ExecutorHandle, SubmitOutcome};
pub use fees::FeeModel;
pub use models::OrderIntent;
pub use portfolio::{equity, marks_from_snapshots, EquityValuation, MarkPrices};