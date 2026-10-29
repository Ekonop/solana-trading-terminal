//! Market provider adapters. Strategy logic consumes validated `ProviderQuote`
//! domain models, never provider-specific JSON.

use async_trait::async_trait;

use crate::errors::AppError;
use crate::market_data::models::ProviderQuote;

pub mod dexscreener;
pub mod mock;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider returned an error: {0}")]
    Remote(String),
    #[error("provider response was malformed: {0}")]
    Malformed(String),
    #[error("request timed out")]
    Timeout,
    #[error("rate limited: {0}")]
    RateLimited(String),
    #[error("cancelled")]
    Cancelled,
}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            ProviderError::Timeout
        } else if e.is_connect() {
            ProviderError::Remote(format!("connection failed: {e}"))
        } else {
            ProviderError::Remote(e.to_string())
        }
    }
}

impl From<ProviderError> for AppError {
    fn from(e: ProviderError) -> Self {
        match e {
            ProviderError::Timeout => AppError::MarketData("provider request timed out".into()),
            ProviderError::Cancelled => AppError::MarketData("request cancelled".into()),
            other => AppError::MarketData(other.to_string()),
        }
    }
}

/// A market-data source. Implementations must never panic and must validate
/// their responses before returning quotes.
#[async_trait]
pub trait MarketProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// True when the provider generates simulated/demo data.
    fn is_simulated(&self) -> bool;
    /// The mint representing SOL (price of SOL in USD) for this provider.
    fn sol_mint(&self) -> &'static str;
    /// Fetch a batch of quotes. Providers should return a bounded set.
    async fn fetch_quotes(&self) -> Result<Vec<ProviderQuote>, ProviderError>;
}

/// Bounded retry with exponential backoff + jitter (never retries 4xx-rate-
/// limited beyond `max_retries`; timeouts are always retried).
pub async fn fetch_with_retry(
    provider: &dyn MarketProvider,
    max_retries: u32,
    backoff_base_ms: u64,
) -> Result<Vec<ProviderQuote>, ProviderError> {
    let mut attempt = 0u32;
    loop {
        match provider.fetch_quotes().await {
            Ok(q) => return Ok(q),
            Err(ProviderError::RateLimited(_)) if attempt >= max_retries => {
                return Err(ProviderError::RateLimited("retries exhausted".into()));
            }
            Err(e) if attempt >= max_retries => return Err(e),
            Err(e) => {
                tracing::warn!(
                    provider = provider.name(),
                    attempt,
                    error = %e,
                    "provider fetch failed, retrying"
                );
                attempt += 1;
                let sleep_ms = backoff_base_ms.saturating_mul(1u64 << attempt.min(6))
                    + rand::random::<u64>() % 200;
                tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
            }
        }
    }
}