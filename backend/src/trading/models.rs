//! Shared trading models.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::{OrderSide, OrderType};

/// A validated order request arriving at the paper executor (from a strategy
/// signal or from the API). Everything the executor needs to apply the trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderIntent {
    pub account_id: String,
    pub strategy_id: Option<String>,
    pub mint: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    /// Notional size in USD cents (strategies propose USD notional; the
    /// executor converts to raw base units at the reference price).
    pub size_usd_cents: i64,
    /// Optional limit price (USD per token) for limit orders.
    pub limit_price: Option<Decimal>,
    pub idempotency_key: String,
    pub correlation_id: String,
    /// Reference price used for sizing (USD per token).
    pub ref_price: Decimal,
    pub ref_snapshot_id: Option<String>,
    pub data_observed_at: i64,
}

/// Quantity + decimals context for a token.
#[derive(Debug, Clone, Copy)]
pub struct TokenDecimals {
    pub decimals: i32,
}

impl Default for TokenDecimals {
    fn default() -> Self {
        TokenDecimals { decimals: 9 }
    }
}

/// Cost/notional math helpers (pure, unit-tested).
pub mod math {
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;

    /// Round half away from zero to an i64 (explicit accounting rule).
    pub fn round_half_up(d: Decimal) -> i64 {
        let rounded = d.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        rounded.to_i64().unwrap_or(0)
    }

    /// Raw base units -> human-readable quantity.
    pub fn raw_to_human(raw: i64, decimals: i32) -> Decimal {
        if decimals <= 0 {
            return Decimal::from(raw);
        }
        Decimal::from(raw) / Decimal::from(10_i64.pow(decimals as u32))
    }

    /// Human-readable quantity -> raw base units (rounded half-up).
    pub fn human_to_raw(human: Decimal, decimals: i32) -> i64 {
        let scaled = human * Decimal::from(10_i64.pow(decimals as u32));
        round_half_up(scaled)
    }

    /// Notional USD for a raw quantity at a price.
    pub fn notional_usd(raw: i64, decimals: i32, price: Decimal) -> Decimal {
        raw_to_human(raw, decimals) * price
    }

    /// USD notional -> raw base units at a price (round half-up).
    pub fn usd_to_raw(usd: Decimal, decimals: i32, price: Decimal) -> i64 {
        if price <= Decimal::ZERO {
            return 0;
        }
        human_to_raw(usd / price, decimals)
    }

    /// USD amount denominated in SOL -> lamports.
    pub fn usd_to_lamports(usd: Decimal, solusd: Decimal) -> i64 {
        if solusd <= Decimal::ZERO {
            return 0;
        }
        round_half_up(usd * Decimal::from(crate::domain::LAMPORTS_PER_SOL) / solusd)
    }

    /// Lamports -> USD (marking only).
    pub fn lamports_to_usd(lamports: i64, solusd: Decimal) -> Decimal {
        Decimal::from(lamports) / Decimal::from(crate::domain::LAMPORTS_PER_SOL) * solusd
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use super::math::*;

    #[test]
    fn half_up_rounding() {
        assert_eq!(round_half_up(Decimal::from_str_exact("1.5").unwrap()), 2);
        assert_eq!(round_half_up(Decimal::from_str_exact("2.5").unwrap()), 3);
        assert_eq!(round_half_up(Decimal::from_str_exact("1.4999").unwrap()), 1);
        assert_eq!(round_half_up(Decimal::from_str_exact("-1.5").unwrap()), -2);
    }

    #[test]
    fn raw_human_roundtrip() {
        assert_eq!(raw_to_human(1_234_567_890, 9), Decimal::from_str_exact("1.234567890").unwrap());
        assert_eq!(human_to_raw(Decimal::from_str_exact("1.234567890").unwrap(), 9), 1_234_567_890);
    }

    #[test]
    fn usd_to_raw_and_back() {
        let price = Decimal::from_str_exact("0.00001234").unwrap();
        let raw = usd_to_raw(Decimal::from_str_exact("100").unwrap(), 9, price);
        assert!(raw > 0);
        // Integer base-unit quantization means the round-trip can be off by
        // at most one lamport-equivalent unit — never by more than ~1e-6 USD.
        let back = raw_to_human(raw, 9) * price;
        let diff = (back - Decimal::from(100)).abs();
        assert!(
            diff < Decimal::from_str_exact("0.000001").unwrap(),
            "round-trip error too large: {diff}"
        );
    }
}