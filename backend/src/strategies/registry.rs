//! Static registry of built-in strategies.

use super::dip_hunter::DipHunter;
use super::momentum::Momentum;
use super::scalper::Scalper;
use super::sniper::Sniper;
use super::traits::Strategy;
use super::whale_tail::WhaleTail;

pub const STRATEGIES: &[&'static dyn Strategy] = &[
    &Sniper,
    &Momentum,
    &DipHunter,
    &WhaleTail,
    &Scalper,
];

pub fn find(id: &str) -> Option<&'static dyn Strategy> {
    STRATEGIES.iter().copied().find(|s| s.id() == id)
}

/// Register all built-in strategies in the database (idempotent upsert).
pub async fn register_all(
    db: &crate::db::Database,
) -> Result<(), crate::errors::AppError> {
    for s in STRATEGIES {
        let default = serde_json::to_string(&s.default_config()).unwrap_or_else(|_| "{}".into());
        crate::repos::strategies::register_strategy(
            db,
            s.id(),
            s.display_name(),
            s.description(),
            &default,
        )
        .await?;
    }
    Ok(())
}