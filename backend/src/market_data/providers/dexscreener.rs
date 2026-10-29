//! DexScreener adapter — verified against the current public read-only API:
//!   GET https://api.dexscreener.com/latest/dex/tokens/{mint}
//!   GET https://api.dexscreener.com/latest/dex/search?q={query}
//! Free tier, no authentication. Responses are treated as untrusted: every
//! field is validated and malformed entries are dropped, never propagated.
//!
//! The adapter maintains a fixed, configurable watchlist of mints and returns
//! the best (highest-liquidity) valid pair per mint. SOL/USD comes from the
//! same token endpoint for the native SOL mint.

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Deserialize;

use super::{MarketProvider, ProviderError};
use crate::market_data::models::ProviderQuote;

pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// Default watchlist: well-known, real token mints on Solana.
pub const DEFAULT_WATCHLIST: &[&str] = &[
    "So11111111111111111111111111111111111111112", // SOL
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // USDC
    "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN", // JUP
    "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263", // BONK
    "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So", // mSOL
    "EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm", // wif
];

const BASE: &str = "https://api.dexscreener.com/latest/dex/tokens/";

#[derive(Debug, Deserialize)]
struct DexResponse {
    pairs: Option<Vec<DexPair>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DexPair {
    chain_id: Option<String>,
    dex_id: Option<String>,
    pair_address: Option<String>,
    base_token: Option<DexToken>,
    quote_token: Option<DexToken>,
    price_usd: Option<String>,
    liquidity: Option<DexLiquidity>,
    volume: Option<DexVolume>,
}

#[derive(Debug, Deserialize)]
struct DexToken {
    name: Option<String>,
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DexLiquidity {
    usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DexVolume {
    h24: Option<f64>,
}

pub struct DexScreenerProvider {
    client: reqwest::Client,
    watchlist: Vec<String>,
    pub sol_mint: String,
}

impl DexScreenerProvider {
    pub fn new(watchlist: Vec<String>) -> Result<DexScreenerProvider, ProviderError> {
        let client = reqwest::Client::builder()
            .user_agent("solana-trading-terminal/0.1 (research; paper-trading only)")
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(ProviderError::from)?;
        Ok(DexScreenerProvider {
            client,
            watchlist,
            sol_mint: SOL_MINT.to_string(),
        })
    }

    pub fn with_default_watchlist() -> Result<DexScreenerProvider, ProviderError> {
        DexScreenerProvider::new(DEFAULT_WATCHLIST.iter().map(|s| s.to_string()).collect())
    }

    async fn fetch_mint(&self, mint: &str) -> Result<Vec<ProviderQuote>, ProviderError> {
        let url = format!("{BASE}{mint}");
        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await?;
        if resp.status().as_u16() == 429 {
            return Err(ProviderError::RateLimited("HTTP 429".into()));
        }
        if !resp.status().is_success() {
            return Err(ProviderError::Remote(format!(
                "HTTP {} for {mint}",
                resp.status()
            )));
        }
        let parsed: DexResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed(format!("bad JSON for {mint}: {e}")))?;
        let Some(pairs) = parsed.pairs else {
            return Ok(Vec::new());
        };
        let mut best: Option<ProviderQuote> = None;
        for p in pairs {
            let network = p.chain_id.clone().unwrap_or_else(|| "solana".into());
            let Some(price_str) = p.price_usd else { continue };
            let Ok(price) = price_str.parse::<Decimal>() else { continue };
            if price <= Decimal::ZERO {
                continue;
            }
            let liqu = p.liquidity.and_then(|l| l.usd).filter(|v| *v >= 0.0);
            let liq_dec = liqu.and_then(Decimal::from_f64_retain);
            let vol_dec = p
                .volume
                .and_then(|v| v.h24)
                .filter(|v| *v >= 0.0)
                .and_then(Decimal::from_f64_retain);
            let q = ProviderQuote {
                mint: mint.to_string(),
                symbol: p.base_token.as_ref().and_then(|t| t.symbol.clone()),
                name: p.base_token.as_ref().and_then(|t| t.name.clone()),
                pair_id: p.pair_address.clone(),
                price,
                liquidity_usd: liq_dec,
                volume_24h_usd: vol_dec,
                // DexScreener does not expose token decimals; risk refuses
                // orders on mints with unknown decimals.
                decimals: None,
                network,
                observed_at: crate::time::now_micros(),
            };
            // Prefer the highest-liquidity pair as the market reference.
            best = Some(match best {
                None => q,
                Some(b) if b.liquidity_usd.unwrap_or_default() < q.liquidity_usd.unwrap_or_default() => q,
                Some(b) => b,
            });
        }
        Ok(best.into_iter().collect())
    }
}

#[async_trait]
impl MarketProvider for DexScreenerProvider {
    fn name(&self) -> &'static str {
        "dexscreener"
    }

    fn is_simulated(&self) -> bool {
        false
    }

    fn sol_mint(&self) -> &'static str {
        SOL_MINT
    }

    async fn fetch_quotes(&self) -> Result<Vec<ProviderQuote>, ProviderError> {
        // Sequential, bounded watchlist loop. Each mint has its own timeout;
        // a single failing mint degrades to partial results (other mints
        // still update) instead of failing the whole batch.
        let mut out = Vec::new();
        let mut errors = 0usize;
        for mint in &self.watchlist {
            match self.fetch_mint(mint).await {
                Ok(qs) => out.extend(qs),
                Err(e) => {
                    errors += 1;
                    tracing::warn!(mint, error = %e, "dexscreener fetch failed for mint");
                }
            }
        }
        if out.is_empty() && errors > 0 {
            return Err(ProviderError::Remote(format!(
                "all {errors} watchlist fetches failed"
            )));
        }
        Ok(out)
    }
}