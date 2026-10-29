//! Shared strategy math helpers. Pure float math for *signal scoring* only —
//! never for accounting, which stays in integer/Decimal territory.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::domain::{MarketSnapshot, Position};

/// A bounded price+volume series for window analysis.
#[derive(Debug, Clone, Default)]
pub struct Series {
    pub prices: Vec<f64>,
    pub volumes: Vec<f64>,
    pub observed_at: Vec<i64>,
}

impl Series {
    /// Build from snapshots, oldest first. Drops non-positive prices.
    pub fn from_snapshots(snaps: &[MarketSnapshot]) -> Series {
        let mut s = Series::default();
        for x in snaps {
            if let Some(p) = x.price.to_f64() {
                if p > 0.0 {
                    s.prices.push(p);
                    s.volumes.push(
                        x.volume_24h_usd
                            .map(|v| v.to_f64().unwrap_or_default())
                            .unwrap_or_default(),
                    );
                    s.observed_at.push(x.observed_at);
                }
            }
        }
        s
    }

    pub fn len(&self) -> usize {
        self.prices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prices.is_empty()
    }

    pub fn last_price(&self) -> Option<f64> {
        self.prices.last().copied()
    }

    pub fn first_price(&self) -> Option<f64> {
        self.prices.first().copied()
    }

    /// Return over `n` bars ending at the last observation.
    pub fn return_over(&self, n: usize) -> Option<f64> {
        if self.len() < n + 1 {
            return None;
        }
        let start = self.prices[self.len() - 1 - n];
        let end = *self.prices.last()?;
        if start <= 0.0 {
            return None;
        }
        Some((end - start) / start)
    }

    /// Simple moving average of the last `n` prices.
    pub fn sma(&self, n: usize) -> Option<f64> {
        if self.len() < n || n == 0 {
            return None;
        }
        let sum: f64 = self.prices[self.len() - n..].iter().sum();
        Some(sum / n as f64)
    }

    /// Rolling local high over the last `n` bars.
    pub fn local_high(&self, n: usize) -> Option<f64> {
        let start = self.len().saturating_sub(n);
        if start >= self.len() {
            return None;
        }
        self.prices[start..].iter().copied().fold(f64::NEG_INFINITY, f64::max).into()
    }

    /// Realized volatility (stdev of log returns) over the last `n` bars.
    pub fn volatility(&self, n: usize) -> Option<f64> {
        if self.len() < n + 1 || n == 0 {
            return None;
        }
        let mut logs = Vec::with_capacity(n);
        for i in self.len() - n..self.len() {
            let a = self.prices[i - 1];
            let b = self.prices[i];
            if a <= 0.0 || b <= 0.0 {
                return None;
            }
            logs.push((b / a).ln());
        }
        let mean = logs.iter().sum::<f64>() / logs.len() as f64;
        let var = logs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / logs.len() as f64;
        Some(var.sqrt())
    }

    /// Relative volume: latest volume vs the average of the previous `n` bars.
    pub fn rel_volume(&self, n: usize) -> Option<f64> {
        if self.len() < n + 1 {
            return None;
        }
        let recent = self.volumes[self.len() - 1];
        let avg: f64 = self.volumes[self.len() - 1 - n..self.len() - 1].iter().sum::<f64>() / n as f64;
        if avg <= 0.0 {
            return None;
        }
        Some(recent / avg)
    }

    /// Median inter-observation interval (ms) — the simulated data cadence.
    pub fn median_interval_ms(&self) -> Option<i64> {
        if self.len() < 2 {
            return None;
        }
        let mut diffs: Vec<i64> = self
            .observed_at
            .windows(2)
            .map(|w| (w[1] - w[0]) / 1_000)
            .filter(|d| *d >= 0)
            .collect();
        if diffs.is_empty() {
            return None;
        }
        diffs.sort_unstable();
        Some(diffs[diffs.len() / 2])
    }
}

/// Convert f64 to Decimal safely for proposal fields.
pub fn dec(f: f64) -> Decimal {
    Decimal::from_f64_retain(f).unwrap_or(Decimal::ZERO)
}

/// Freshness gate used by every strategy: a mint is eligible when its latest
/// snapshot is younger than `max_age_ms`.
pub fn is_fresh(snap: &MarketSnapshot, max_age_ms: u64) -> bool {
    !snap.is_stale(max_age_ms) && snap.price > Decimal::ZERO
}

/// Compute a position's current USD value for a mint.
pub fn position_value_usd(positions: &[Position], mint: &str, price: Decimal) -> Decimal {
    positions
        .iter()
        .find(|p| p.mint == mint)
        .map(|p| crate::trading::models::math::raw_to_human(p.qty_raw, 9) * price)
        .unwrap_or(Decimal::ZERO)
}

/// Default proposal size from equity (fraction of equity in USD cents).
pub fn size_usd_cents(equity_usd: Decimal, fraction: f64) -> i64 {
    let size = equity_usd * Decimal::from_f64_retain(fraction).unwrap_or(Decimal::ZERO);
    (size * Decimal::from(100)).round_dp(0).to_i64().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(price: f64, vol: f64, ts: i64) -> MarketSnapshot {
        MarketSnapshot {
            id: "".into(),
            mint: "m".into(),
            symbol: None,
            name: None,
            pair_id: None,
            price: Decimal::from_f64_retain(price).unwrap(),
            liquidity_usd: None,
            volume_24h_usd: Decimal::from_f64_retain(vol),
            source: "t".into(),
            is_simulated: true,
            observed_at: ts,
            received_at: ts,
            quality: 100,
        }
    }

    #[test]
    fn return_over_windows() {
        let snaps = vec![
            snap(100.0, 10.0, 1),
            snap(110.0, 12.0, 2),
            snap(99.0, 30.0, 3),
        ];
        let s = Series::from_snapshots(&snaps);
        assert_eq!(s.len(), 3);
        assert!((s.return_over(2).unwrap() - (-0.01)).abs() < 1e-9);
        assert_eq!(s.local_high(3).unwrap(), 110.0);
        assert!(s.rel_volume(2).unwrap() > 1.0);
    }

    #[test]
    fn median_interval() {
        let snaps = vec![snap(1.0, 1.0, 0), snap(1.0, 1.0, 15_000_000), snap(1.0, 1.0, 30_000_000)];
        let s = Series::from_snapshots(&snaps);
        assert_eq!(s.median_interval_ms(), Some(15_000));
    }
}