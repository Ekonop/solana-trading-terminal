//! Simulated fee model.
//!
//! Current model: fixed basis points of the traded notional, charged in SOL
//! lamports at the simulated fill (converted via the SOL/USD reference).
//! Fees are always integer lamports with explicit half-away-from-zero
//! rounding; they are never negative and never < 0 (fee_bps = 0 allowed).

use rust_decimal::Decimal;

use crate::config::PaperTradingConfig;
use crate::trading::models::math;

#[derive(Debug, Clone, Copy)]
pub struct FeeModel {
    /// Fee in basis points (1 bp = 0.0001 = 0.01%).
    pub fee_bps: i64,
}

impl FeeModel {
    pub fn from_config(cfg: &PaperTradingConfig) -> FeeModel {
        FeeModel {
            fee_bps: cfg.fee_bps,
        }
    }

    pub fn name(&self) -> String {
        format!("fixed-bps({})", self.fee_bps)
    }

    /// Simulated fee in lamports for a notional amount of USD.
    pub fn fee_lamports(&self, notional_usd: Decimal, solusd: Decimal) -> i64 {
        if self.fee_bps == 0 {
            return 0;
        }
        let notional_lamports = math::usd_to_lamports(notional_usd, solusd);
        let fee = Decimal::from(notional_lamports) * Decimal::from(self.fee_bps)
            / Decimal::from(10_000);
        math::round_half_up(fee)
    }

    /// Simulated fee in USD for a notional amount of USD.
    pub fn fee_usd(&self, notional_usd: Decimal) -> Decimal {
        notional_usd * Decimal::from(self.fee_bps) / Decimal::from(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_zero_when_bps_zero() {
        let m = FeeModel { fee_bps: 0 };
        assert_eq!(m.fee_lamports(Decimal::from(1_000u64), Decimal::from(150u64)), 0);
    }

    #[test]
    fn fee_25bps_of_notional() {
        let m = FeeModel { fee_bps: 25 };
        // notional $1000 at SOL $150 -> 6.6667 SOL = 6_666_666_667 lamports
        // 0.25% -> 16_666_667 lamports (rounded)
        let solusd = Decimal::from(150u64);
        let lamports = m.fee_lamports(Decimal::from(1_000u64), solusd);
        assert!((lamports - 16_666_667).abs() <= 2, "got {lamports}");
        assert_eq!(m.fee_usd(Decimal::from(1_000u64)), Decimal::from_str_exact("2.5").unwrap());
    }
}