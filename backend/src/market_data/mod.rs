//! Market-data subsystem: provider adapters, ingestion, freshness validation.

pub mod cache;
pub mod models;
pub mod providers;
pub mod service;

pub use cache::MarketCache;
pub use models::{ProviderHealth, ProviderQuote};
pub use service::{MarketDataService, MarketDataServiceHandle};