//! Deterministic demo provider.
//!
//! Generates clearly-labelled simulated observations (`is_simulated = true`,
//! source `demo`) for a fixed universe of tokens using a seeded ChaCha12 RNG
//! plus deterministic market dynamics: sinusoidal price waves, a slowly
//! mean-reverting price regime, and occasional volume bursts (so strategies
//! that require relative-volume spikes actually get to fire). It never
//! contacts the network and produces repeatable series for a fixed seed and
//! tick index: every draw is a pure function of `(seed, tick, token index)`.
//!
//! This is research sample data. It is never mislabelled as live market data:
//! the API, WebSocket events, and the UI all carry the `simulated` flag.

use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
use rust_decimal::Decimal;

use super::{MarketProvider, ProviderError};
use crate::market_data::models::ProviderQuote;

/// Mint of the demo SOL reference rate.
pub const SOL_MINT: &str = "so11111111111111111111111111111111111111112";
const DEMO_SEED: u64 = 0x534f4c41; // "SOLA"

/// Deterministic splitmix64-style mix (pure function, no RNG state).
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

/// Deterministic 64-bit hash of (seed, token index, tick scalar).
#[inline]
fn hash3(seed: u64, tok_idx: u64, tick: u64) -> u64 {
    mix(seed ^ mix(tok_idx).wrapping_add(0x9e3779b9).wrapping_mul(tick))
}

/// Deterministic price-regime drift for a token at a tick, in [-1, 1) units
/// that get scaled per token. Mean-reverts: tends back toward 0.
#[inline]
fn regime_drift(seed: u64, tok_idx: u64, tick: u64) -> f64 {
    // ±1 in 2048 steps, mapped to roughly a bounded random walk.
    let h = hash3(seed, tok_idx, tick);
    ((h % 2049) as f64 - 1024.0) / 1024.0
}

struct DemoToken {
    mint: &'static str,
    symbol: &'static str,
    name: &'static str,
    decimals: i32,
    base_price: f64,
    amplitude: f64,
    period_secs: f64,
    phase: f64,
    liquidity_usd: f64,
    volume_24h_usd: f64,
}

const DEMO_TOKENS: &[DemoToken] = &[
    DemoToken { mint: SOL_MINT, symbol: "SOL", name: "Solana", decimals: 9, base_price: 148.0, amplitude: 0.05, period_secs: 600.0, phase: 0.0, liquidity_usd: 8_000_000.0, volume_24h_usd: 400_000_000.0 },
    DemoToken { mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", symbol: "USDC", name: "USD Coin", decimals: 6, base_price: 1.00, amplitude: 0.001, period_secs: 900.0, phase: 1.0, liquidity_usd: 5_000_000.0, volume_24h_usd: 250_000_000.0 },
    DemoToken { mint: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263", symbol: "BONK", name: "Bonk", decimals: 5, base_price: 0.000029, amplitude: 0.14, period_secs: 180.0, phase: 2.0, liquidity_usd: 1_200_000.0, volume_24h_usd: 18_000_000.0 },
    DemoToken { mint: "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN", symbol: "JUP", name: "Jupiter", decimals: 6, base_price: 0.92, amplitude: 0.09, period_secs: 240.0, phase: 3.0, liquidity_usd: 2_400_000.0, volume_24h_usd: 35_000_000.0 },
    DemoToken { mint: "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So", symbol: "mSOL", name: "Marinade Staked SOL", decimals: 9, base_price: 159.0, amplitude: 0.03, period_secs: 420.0, phase: 4.0, liquidity_usd: 900_000.0, volume_24h_usd: 9_000_000.0 },
    DemoToken { mint: "7dHbWXmci3dT8UFYWYZweBLXgycu7Y3iL6trKn1Y7ARj", symbol: "LST", name: "Liquid Staking Token", decimals: 9, base_price: 154.0, amplitude: 0.06, period_secs: 300.0, phase: 5.0, liquidity_usd: 700_000.0, volume_24h_usd: 6_000_000.0 },
    // This token only appears after a few ticks — used by the Sniper strategy
    // to exercise the "newly observed pair" path. Deterministic appearance.
    DemoToken { mint: "FNxJ4FTPjDhaqYbP9N3wrBoRgixCa6xjnB7YaB1pPB26", symbol: "NEPT", name: "Neptune (demo new pair)", decimals: 9, base_price: 0.004, amplitude: 0.25, period_secs: 45.0, phase: 6.0, liquidity_usd: 60_000.0, volume_24h_usd: 2_100_000.0 },
];

pub struct MockProvider {
    tick: Mutex<u64>,
    rng: Mutex<ChaCha12Rng>,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProvider {
    pub fn new() -> MockProvider {
        MockProvider {
            tick: Mutex::new(0),
            rng: Mutex::new(ChaCha12Rng::seed_from_u64(DEMO_SEED)),
        }
    }

    fn now_micros() -> i64 {
        let since = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        since.as_micros() as i64
    }
}

#[async_trait]
impl MarketProvider for MockProvider {
    fn name(&self) -> &'static str {
        "demo"
    }

    fn is_simulated(&self) -> bool {
        true
    }

    fn sol_mint(&self) -> &'static str {
        SOL_MINT
    }

    async fn fetch_quotes(&self) -> Result<Vec<ProviderQuote>, ProviderError> {
        let mut tick = self.tick.lock().unwrap();
        *tick += 1;
        let this_tick = *tick;
        let now = Self::now_micros();
        // Price phase is driven by the deterministic tick index, NOT wall
        // clock: two instances at the same tick produce identical quotes
        // (the documented determinism guarantee). Timestamps still use the
        // real clock so freshness rules and the UI behave realistically.
        let t = this_tick as f64;

        let mut quotes = Vec::with_capacity(DEMO_TOKENS.len());
        for (tok_idx, tok) in DEMO_TOKENS.iter().enumerate() {
            // New-pair token appears only from tick 4 onward (deterministic).
            if tok.symbol == "NEPT" && this_tick < 4 {
                continue;
            }
            let wave = 1.0
                + tok.amplitude * (t / tok.period_secs * std::f64::consts::TAU + tok.phase).sin();
            {
                let mut rng = self.rng.lock().unwrap();
                let noise = rng.gen_range(-0.002..0.002);
                // Price regime: small bounded mean-reverting drift so trends
                // exist but bars stay confirmable (momentum's 2-bar gate).
                let drift = regime_drift(DEMO_SEED, tok_idx as u64, this_tick)
                    * tok.amplitude
                    * 0.35;
                let price_f =
                    (tok.base_price * wave * (1.0 + drift) + tok.base_price * noise).max(1e-12);
                let liquidity_f = (tok.liquidity_usd * (1.0 + rng.gen_range(-0.01..0.01))).max(1_000.0);
                // Volume: narrow per-tick bursts (1-2 bars). rel_volume = recent /
                // trailing average, so a burst must be SHORTER than the baseline
                // window or the average catches up and the ratio flattens.
                // A burst rolls on a per-tick hash; the next tick often returns
                // to baseline. ~5-20% of bars per token are bursts.
                let burst_on = {
                    let h = hash3(DEMO_SEED, tok_idx as u64, this_tick);
                    let chance = 0.06 + tok.amplitude * 0.5; // ~6%..18%
                    let prev = hash3(DEMO_SEED, tok_idx as u64, this_tick.saturating_sub(1));
                    (h % 1000) as f64 / 1000.0 < chance || (prev % 1000) as f64 / 1000.0 < chance
                };
                let burst = if burst_on {
                    // 1.6x..6x depending on hash + amplitude.
                    let h2 = hash3(DEMO_SEED ^ 0x9E3779B9, tok_idx as u64, this_tick);
                    1.6 + (h2 >> 32) as f64 / u32::MAX as f64 * 4.0 * (0.5 + tok.amplitude)
                } else {
                    1.0
                };
                let volume_f = (tok.volume_24h_usd * burst * (1.0 + rng.gen_range(-0.03..0.03)))
                    .max(1_000.0);

                let price = Decimal::from_f64_retain(price_f)
                    .unwrap_or_else(|| Decimal::from(1u64))
                    .round_dp(12);
                let liquidity = Decimal::from_f64_retain(liquidity_f).map(|d| d.round_dp(2));
                let volume = Decimal::from_f64_retain(volume_f).map(|d| d.round_dp(2));

                quotes.push(ProviderQuote {
                    mint: tok.mint.to_string(),
                    symbol: Some(tok.symbol.to_string()),
                    name: Some(tok.name.to_string()),
                    pair_id: None,
                    price,
                    liquidity_usd: liquidity,
                    volume_24h_usd: volume,
                    decimals: Some(tok.decimals),
                    observed_at: now,
                });
            }
        }
        Ok(quotes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deterministic_same_seed_same_prices() {
        let a = MockProvider::new();
        let b = MockProvider::new();
        let qa = a.fetch_quotes().await.unwrap();
        let qb = b.fetch_quotes().await.unwrap();
        assert_eq!(qa.len(), qb.len());
        for (x, y) in qa.iter().zip(qb.iter()) {
            assert_eq!(x.price, y.price, "same tick must produce same price for {}", x.symbol.as_deref().unwrap_or("?"));
            assert_eq!(x.symbol, y.symbol);
        }
        // SOLUSD is present and positive
        let sol = qa.iter().find(|q| q.mint == SOL_MINT).unwrap();
        assert!(sol.price > Decimal::ZERO);
    }

    #[tokio::test]
    async fn tick_advances() {
        let a = MockProvider::new();
        let _ = a.fetch_quotes().await.unwrap();
        let _ = a.fetch_quotes().await.unwrap();
        let _ = a.fetch_quotes().await.unwrap();
        let qa = a.fetch_quotes().await.unwrap();
        // NEPT appears deterministically at tick >= 4
        assert!(qa.iter().any(|q| q.symbol.as_deref() == Some("NEPT")));
    }

    /// The demo feed must produce real relative-volume spikes (recent volume /
    /// trailing average > 1.5) so volume-driven strategies actually fire.
    #[tokio::test]
    async fn volume_bursts_produce_rel_volume_spikes() {
        use rust_decimal::prelude::ToPrimitive;

        // Collect a series of BONK quotes over many ticks and measure the
        // ratio of each bar's volume to the average of the 8 prior bars.
        let p = MockProvider::new();
        let mut history: Vec<f64> = Vec::new();
        let mut max_rel_vol = 0.0f64;
        for _ in 0..600 {
            let quotes = p.fetch_quotes().await.unwrap();
            let bonk = quotes
                .iter()
                .find(|q| q.symbol.as_deref() == Some("BONK"))
                .unwrap();
            let v = bonk.volume_24h_usd.unwrap().to_f64().unwrap();
            if history.len() >= 8 {
                let avg: f64 = history[history.len() - 8..].iter().sum::<f64>() / 8.0;
                if avg > 0.0 {
                    max_rel_vol = max_rel_vol.max(v / avg);
                }
            }
            history.push(v);
        }
        assert!(
            max_rel_vol > 1.5,
            "expected a real volume spike, got max rel-vol {max_rel_vol}"
        );
    }
}
