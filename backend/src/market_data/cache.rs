//! In-memory market cache: latest snapshots + bounded history rings, plus a
//! provider-health slot. Written by the ingestion loop, read by strategies,
//! the executor, the equity sampler, and the API.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use rust_decimal::Decimal;
use tokio::sync::RwLock;

use crate::domain::MarketSnapshot;

use super::models::ProviderHealth;

pub const HISTORY_RING_CAPACITY: usize = 2000;

#[derive(Debug, Clone)]
pub struct MarketCache {
    inner: Arc<RwLock<MarketCacheInner>>,
}

#[derive(Debug)]
struct MarketCacheInner {
    /// mint -> latest snapshot
    latest: HashMap<String, MarketSnapshot>,
    /// mint -> bounded history (oldest first)
    history: HashMap<String, VecDeque<MarketSnapshot>>,
    health: ProviderHealth,
    /// Total snapshot count ingested this process.
    total_ingested: u64,
}

impl Default for MarketCacheInner {
    fn default() -> Self {
        MarketCacheInner {
            latest: HashMap::new(),
            history: HashMap::new(),
            health: ProviderHealth::new("none", true),
            total_ingested: 0,
        }
    }
}

impl MarketCache {
    pub fn new(initial_health: ProviderHealth) -> MarketCache {
        MarketCache {
            inner: Arc::new(RwLock::new(MarketCacheInner {
                health: initial_health,
                ..Default::default()
            })),
        }
    }

    /// Insert a validated snapshot into the cache (latest + ring).
    pub async fn insert(&self, s: MarketSnapshot) {
        let mut g = self.inner.write().await;
        g.total_ingested += 1;
        g.latest.insert(s.mint.clone(), s.clone());
        g.history
            .entry(s.mint.clone())
            .or_default()
            .push_back(s);
        // Trim every ring to the bounded capacity.
        for ring in g.history.values_mut() {
            while ring.len() > HISTORY_RING_CAPACITY {
                ring.pop_front();
            }
        }
    }

    pub async fn latest_for(&self, mint: &str) -> Option<MarketSnapshot> {
        self.inner.read().await.latest.get(mint).cloned()
    }

    pub async fn latest_all(&self) -> Vec<MarketSnapshot> {
        let g = self.inner.read().await;
        let mut v: Vec<MarketSnapshot> = g.latest.values().cloned().collect();
        v.sort_by_key(|a| std::cmp::Reverse(a.observed_at));
        v
    }

    pub async fn has_mint(&self, mint: &str) -> bool {
        self.inner.read().await.latest.contains_key(mint)
    }

    /// History ring for a mint (oldest first). Bounded.
    pub async fn history_for(&self, mint: &str, max: usize) -> Vec<MarketSnapshot> {
        let g = self.inner.read().await;
        g.history
            .get(mint)
            .map(|ring| {
                let skip = ring.len().saturating_sub(max.max(2));
                ring.iter().skip(skip).cloned().collect()
            })
            .unwrap_or_default()
    }

    pub async fn sol_price(&self, sol_mint: &str) -> Option<Decimal> {
        self.latest_for(sol_mint).await.map(|s| s.price)
    }

    pub async fn health(&self) -> ProviderHealth {
        self.inner.read().await.health.clone()
    }

    pub async fn set_health(&self, h: ProviderHealth) {
        self.inner.write().await.health = h;
    }

    pub async fn total_ingested(&self) -> u64 {
        self.inner.read().await.total_ingested
    }
}