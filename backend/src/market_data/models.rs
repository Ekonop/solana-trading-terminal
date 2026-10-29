//! Provider-agnostic market data models.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::MarketSnapshot;

/// One validated quote from a provider adapter, before persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProviderQuote {
    pub mint: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub pair_id: Option<String>,
    pub price: Decimal,
    pub liquidity_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub decimals: Option<i32>,
    /// Provider observation time (micros). Filled by the provider or now().
    pub observed_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    Unknown,
    Healthy,
    Degraded,
    Down,
}

/// Health of the market-data provider as seen by the ingestion loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProviderHealth {
    pub provider: String,
    pub simulated: bool,
    pub state: ProviderState,
    /// Micros of the last successful fetch (None if never).
    pub last_success: Option<i64>,
    /// Micros of the last attempt.
    pub last_attempt: Option<i64>,
    /// Consecutive failures.
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
}

impl ProviderHealth {
    pub fn new(provider: &str, simulated: bool) -> ProviderHealth {
        ProviderHealth {
            provider: provider.to_string(),
            simulated,
            state: ProviderState::Unknown,
            last_success: None,
            last_attempt: None,
            consecutive_failures: 0,
            last_error: None,
        }
    }

    pub fn record_success(&mut self, at: i64) {
        self.last_success = Some(at);
        self.last_attempt = Some(at);
        self.consecutive_failures = 0;
        self.last_error = None;
        self.state = ProviderState::Healthy;
    }

    pub fn record_failure(&mut self, at: i64, err: &str) {
        self.last_attempt = Some(at);
        self.consecutive_failures += 1;
        self.last_error = Some(err.to_string());
        self.state = if self.consecutive_failures >= 5 {
            ProviderState::Down
        } else if self.consecutive_failures >= 2 {
            ProviderState::Degraded
        } else {
            ProviderState::Healthy
        };
    }

    pub fn is_up(&self) -> bool {
        matches!(self.state, ProviderState::Healthy | ProviderState::Degraded)
            || (self.state == ProviderState::Unknown && self.simulated)
    }
}

/// Convert a provider quote into a persisted snapshot with quality metadata.
pub fn quote_to_snapshot(q: &ProviderQuote, source: &str, simulated: bool, now: i64) -> MarketSnapshot {
    let quality: i32 = if simulated {
        100
    } else if q.price > Decimal::ZERO
        && q.liquidity_usd.unwrap_or(Decimal::ZERO) > Decimal::ZERO
        && q.volume_24h_usd.unwrap_or(Decimal::ZERO) > Decimal::ZERO
    {
        90
    } else if q.price > Decimal::ZERO {
        70
    } else {
        30
    };
    MarketSnapshot {
        id: crate::ids::new_id(),
        mint: q.mint.clone(),
        symbol: q.symbol.clone(),
        name: q.name.clone(),
        pair_id: q.pair_id.clone(),
        price: q.price,
        liquidity_usd: q.liquidity_usd,
        volume_24h_usd: q.volume_24h_usd,
        source: source.to_string(),
        is_simulated: simulated,
        observed_at: q.observed_at,
        received_at: now,
        quality,
    }
}