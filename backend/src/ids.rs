//! Small identifiers.

use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Deterministic idempotency key for an order derived from a signal.
pub fn signal_idempotency_key(signal_id: &str) -> String {
    format!("signal:{signal_id}")
}

/// Deterministic idempotency key derived from a proposal's identity fields.
/// Re-evaluations of the exact same proposal deduplicate; distinct
/// proposals (different strategy, mint, or observation time) never collide.
pub fn proposal_idempotency_key(p: &crate::domain::SignalProposal) -> String {
    format!("sig:{}:{}:{}", p.strategy_id, p.mint, p.data_observed_at)
}