//! Portfolio valuation: cash + marked positions = equity.
//!
//! Cash is integer lamports. Positions are marked to the latest validated
//! snapshot of each mint. Valuations use `rust_decimal`; floats exist only at
//! the presentation boundary.

use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::domain::{Account, MarketSnapshot, Position};

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityValuation {
    pub cash_lamports: i64,
    pub positions_value_usd: Decimal,
    pub equity_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    /// Fraction of equity currently held in positions (0..1).
    pub exposure_pct: Decimal,
    pub solusd: Decimal,
}

#[derive(Debug, Clone, Default)]
pub struct MarkPrices {
    /// mint -> price (USD per token)
    pub prices: HashMap<String, Decimal>,
    pub solusd: Decimal,
}

impl MarkPrices {
    pub fn price_of(&self, mint: &str) -> Option<Decimal> {
        self.prices.get(mint).copied()
    }
}

/// Marks positions against a price map.
pub fn value_positions(
    positions: &[Position],
    marks: &MarkPrices,
) -> (Decimal, Decimal) {
    // (total value USD, unrealized pnl USD)
    let mut value = Decimal::ZERO;
    let mut unrealized = Decimal::ZERO;
    for p in positions {
        if p.qty_raw <= 0 {
            continue;
        }
        if let Some(price) = marks.price_of(&p.mint) {
            let human = crate::trading::models::math::raw_to_human(p.qty_raw, 9);
            value += human * price;
            unrealized += human * (price - p.avg_entry_price);
        }
        // A position with no price contributes 0 to marked value; the caller
        // decides how to surface "valuation unavailable".
    }
    (value, unrealized)
}

/// Full equity valuation for an account.
pub fn equity(
    account: &Account,
    positions: &[Position],
    marks: &MarkPrices,
) -> EquityValuation {
    let (positions_value, unrealized) = value_positions(positions, marks);
    let cash_usd = crate::trading::models::math::lamports_to_usd(account.cash_balance_lamports, marks.solusd);
    let equity_usd = cash_usd + positions_value;
    let exposure_pct = if equity_usd > Decimal::ZERO {
        positions_value / equity_usd
    } else {
        Decimal::ZERO
    };
    EquityValuation {
        cash_lamports: account.cash_balance_lamports,
        positions_value_usd: positions_value,
        equity_usd,
        unrealized_pnl_usd: unrealized,
        exposure_pct,
        solusd: marks.solusd,
    }
}

/// Build a mark-price map from latest snapshots skewering the SOL mint.
pub fn marks_from_snapshots(snaps: &[MarketSnapshot], sol_mint: &str) -> MarkPrices {
    let mut prices = HashMap::new();
    let mut solusd = Decimal::ZERO;
    for s in snaps {
        if s.price > Decimal::ZERO {
            prices.insert(s.mint.clone(), s.price);
            if s.mint == sol_mint {
                solusd = s.price;
            }
        }
    }
    MarkPrices { prices, solusd }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Account;

    fn acct(cash: i64) -> Account {
        Account {
            id: "a".into(),
            name: "t".into(),
            currency: "SOL".into(),
            initial_balance_lamports: cash,
            cash_balance_lamports: cash,
            realized_pnl_lamports: 0,
            status: "active".into(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn equity_is_cash_plus_marked_positions() {
        let marks = MarkPrices {
            prices: HashMap::from([("bonk".to_string(), Decimal::from_str_exact("0.000002").unwrap())]),
            solusd: Decimal::from(150u64),
        };
        let positions = vec![Position {
            account_id: "a".into(),
            mint: "bonk".into(),
            qty_raw: 50_000_000_000, // 50 BONK at 9 decimals
            avg_entry_price: Decimal::from_str_exact("0.000001").unwrap(),
            status: "open".into(),
            opened_at: 0,
            updated_at: 0,
        }];
        // cash $150 = 1 SOL => equity = 150 + (50 * 0.000002 = 0.0001) = 150.0001
        let v = equity(&acct(1_000_000_000), &positions, &marks);
        assert_eq!(v.equity_usd, Decimal::from_str_exact("150.000100").unwrap() + Decimal::ZERO);
        assert_eq!(v.unrealized_pnl_usd, Decimal::from_str_exact("0.000050").unwrap());
    }
}