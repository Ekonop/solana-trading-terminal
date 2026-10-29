//! Performance analytics computed from the portfolio ledger and equity
//! snapshots. All money math is Decimal; f64 only for display ratios.

use rust_decimal::Decimal;

use crate::domain::EquitySnapshot;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityStats {
    pub current_equity_usd: Decimal,
    pub positions_value_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub peak_equity_usd: Decimal,
    pub drawdown_pct: Decimal,
    /// Net return vs the first snapshot.
    pub net_return_pct: Decimal,
    pub day_return_pct: Option<Decimal>,
    pub sample_points: usize,
}

/// Compute statistics over an equity series (oldest first).
pub fn equity_stats(series: &[EquitySnapshot]) -> EquityStats {
    let mut stats = EquityStats::default();
    stats.sample_points = series.len();
    let Some(_first) = series.first() else {
        return stats;
    };
    // Ignore placeholder snapshots with zero equity (recorded before the first
    // market quote arrived): they are not real account levels and would poison
    // peak / drawdown / net-return math.
    let valid: Vec<&EquitySnapshot> =
        series.iter().filter(|s| s.equity_usd > Decimal::ZERO).collect();
    let Some(first_valid) = valid.first() else {
        return stats;
    };
    let last_valid = *valid.last().unwrap();

    stats.current_equity_usd = last_valid.equity_usd;
    stats.positions_value_usd = last_valid.positions_value_usd;
    stats.unrealized_pnl_usd = last_valid.unrealized_pnl_usd;

    let peak = valid.iter().map(|s| s.equity_usd).fold(Decimal::ZERO, Decimal::max);
    stats.peak_equity_usd = peak;
    // Max drawdown over the whole series (rolling peak), not just peak-vs-last.
    let mut rolling_peak = Decimal::ZERO;
    let mut max_dd = Decimal::ZERO;
    for s in &valid {
        rolling_peak = rolling_peak.max(s.equity_usd);
        if rolling_peak > Decimal::ZERO {
            max_dd = max_dd.max((rolling_peak - s.equity_usd) / rolling_peak);
        }
    }
    stats.drawdown_pct = max_dd;
    stats.net_return_pct =
        (last_valid.equity_usd - first_valid.equity_usd) / first_valid.equity_usd;
    stats
}

/// Win-rate and profit-factor style metrics from a list of realized PnLs
/// (in dollars or lamports — consistent unit required).
pub fn trade_stats(pnls: &[Decimal]) -> (Option<Decimal>, Option<Decimal>, Option<Decimal>) {
    let wins: Vec<&Decimal> = pnls.iter().filter(|p| **p > Decimal::ZERO).collect();
    let losses: Vec<&Decimal> = pnls.iter().filter(|p| **p < Decimal::ZERO).collect();
    let win_rate = if pnls.is_empty() {
        None
    } else {
        Some(
            (Decimal::from(wins.len()) / Decimal::from(pnls.len())).round_dp(6),
        )
    };
    let avg_win = if wins.is_empty() {
        None
    } else {
        Some(wins.iter().copied().sum::<Decimal>() / Decimal::from(wins.len()))
    };
    let avg_loss = if losses.is_empty() {
        None
    } else {
        Some(losses.iter().copied().sum::<Decimal>() / Decimal::from(losses.len()))
    };
    let _profit_factor = match (avg_win, avg_loss) {
        (Some(w), Some(l)) if l < Decimal::ZERO => Some((w / -l).round_dp(3)),
        (Some(_), None) => Some(Decimal::MAX), // no losses
        _ => None,
    };
    (win_rate, avg_win, avg_loss)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(equity: &str, cash: i64) -> EquitySnapshot {
        EquitySnapshot {
            id: "".into(),
            account_id: "a".into(),
            ts: 0,
            cash_lamports: cash,
            positions_value_usd: Decimal::ZERO,
            equity_usd: Decimal::from_str_exact(equity).unwrap(),
            unrealized_pnl_usd: Decimal::ZERO,
            realized_pnl_lamports: 0,
            exposure_pct: Decimal::ZERO,
        }
    }

    #[test]
    fn stats_compute_drawdown_and_return() {
        let series = vec![snap("100", 0), snap("120", 0), snap("90", 0), snap("110", 0)];
        let s = equity_stats(&series);
        assert_eq!(s.net_return_pct, Decimal::from_str_exact("0.1").unwrap());
        assert_eq!(s.drawdown_pct, Decimal::from_str_exact("0.25").unwrap());
        assert_eq!(s.peak_equity_usd, Decimal::from(120));
    }

    #[test]
    fn stats_ignore_zero_equity_placeholders() {
        // Pre-quote snapshots with equity 0 must not poison peak/drawdown/return.
        let series = vec![
            snap("0", 0),
            snap("0", 0),
            snap("100", 0),
            snap("120", 0),
            snap("90", 0),
            snap("110", 0),
        ];
        let s = equity_stats(&series);
        assert_eq!(s.net_return_pct, Decimal::from_str_exact("0.1").unwrap());
        assert_eq!(s.drawdown_pct, Decimal::from_str_exact("0.25").unwrap());
        assert_eq!(s.peak_equity_usd, Decimal::from(120));
        assert_eq!(s.current_equity_usd, Decimal::from(110));
        assert_eq!(s.sample_points, 6); // raw count kept honest
    }

    #[test]
    fn trade_stats_winrate_and_pf() {
        let pnls = vec![Decimal::from(10), Decimal::from(-5), Decimal::from(15)];
        let (wr, aw, al) = trade_stats(&pnls);
        assert_eq!(wr, Some(Decimal::from_str_exact("0.666667").unwrap()));
        assert_eq!(aw, Some(Decimal::from_str_exact("12.5").unwrap()));
        assert_eq!(al, Some(Decimal::from(-5)));
    }
}