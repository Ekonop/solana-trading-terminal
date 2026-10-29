//! Backtesting / replay engine.
//!
//! Deterministic historical replay: snapshots within [start, end] are fed to
//! the *same* strategy interface and the same atomic fill math used by paper
//! trading, against an isolated in-memory portfolio. Look-ahead is prevented
//! structurally: an event is evaluated only with observations whose
//! `observed_at <= event.observed_at` (the replay cache feeds each snapshot
//! before the evaluation at that timestamp, never after).
//!
//! Formatting caveat (`docs/trading-model.md`): current deployed providers
//! cannot supply long reliable historical series; the demo generator persists
//! its observations as it runs, so backtests replay *real persisted demo
//! observations* (clearly labelled). Runs over too little data are marked
//! "not qualified" instead of showing misleading performance figures.

use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;

use crate::db::Database;
use crate::domain::*;
use crate::errors::AppError;
use crate::market_data::cache::MarketCache;
use crate::market_data::models::ProviderHealth;
use crate::repos;
use crate::strategies::registry;
use crate::trading::fees::FeeModel;
use crate::trading::models::math::{human_to_raw, lamports_to_usd, raw_to_human, usd_to_lamports};

pub struct BacktestRequest {
    pub strategy_id: String,
    pub config_json: String,
    pub interval_start: i64,
    pub interval_end: i64,
    pub initial_lamports: i64,
    pub fee_bps: i64,
    pub slippage_bps: i64,
    pub sol_mint: String,
}

struct SimPortfolio {
    cash_lamports: i64,
    positions: HashMap<String, Position>,
}

/// Run a backtest to completion. Updates the run row as it goes.
pub async fn run_backtest(
    db: Database,
    run_id: String,
    req: BacktestRequest,
) -> Result<BacktestRun, AppError> {
    let strategy = registry::find(&req.strategy_id)
        .ok_or_else(|| AppError::NotFound(format!("strategy {}", req.strategy_id)))?;
    let cfg: serde_json::Value = serde_json::from_str(&req.config_json)
        .map_err(|e| AppError::Strategy(format!("config invalid: {e}")))?;
    strategy.validate_config(&cfg)?;

    repos::backtests::set_backtest_running(&db, &run_id).await?;
    let fee_model = FeeModel { fee_bps: req.fee_bps };

    // Collect chronological snapshots across every mint in range.
    let coverage = repos::markets::mint_coverage(&db).await?;
    let mut events: Vec<MarketSnapshot> = Vec::new();
    for (mint, _, _, _) in coverage {
        if mint == req.sol_mint {
            continue; // SOL rate is a valuation input, not a tradeable here
        }
        events.extend(
            repos::markets::snapshots_in_range(&db, &mint, req.interval_start, req.interval_end)
                .await?,
        );
    }
    events.sort_by_key(|s| s.observed_at);
    // Include SOL snapshots too, as valuation inputs for the currency leg.
    let mut sol_events = repos::markets::snapshots_in_range(
        &db,
        &req.sol_mint,
        req.interval_start,
        req.interval_end,
    )
    .await?;
    sol_events.sort_by_key(|s| s.observed_at);
    // Initial SOL rate: first SOL observation in the replay window (or 150).
    let start_solusd = sol_events
        .first()
        .map(|s| s.price)
        .unwrap_or_else(|| Decimal::from(150));
    for s in sol_events {
        events.push(s);
    }
    events.sort_by_key(|s| s.observed_at);

    let replay_cache = Arc::new(MarketCache::new(ProviderHealth::new("replay", true)));
    let mut portfolio = SimPortfolio {
        cash_lamports: req.initial_lamports,
        positions: HashMap::new(),
    };

    let mut realized_pnls: Vec<Decimal> = Vec::new();
    let mut equity_curve: Vec<Decimal> = Vec::new();
    let mut signals = 0i64;
    let mut rejections = 0i64;
    let mut fee_total_usd = Decimal::ZERO;
    let mut slippage_total_usd = Decimal::ZERO;
    let mut in_market_bars = 0usize;

    for ev in events.iter() {
        // Insert into the replay cache FIRST: the evaluation at this instant
        // may only see observations at or before `now`.
        replay_cache.insert(ev.clone()).await;

        let now = ev.observed_at;
        let snaps = replay_cache.latest_all().await;
        let positions_vec: Vec<Position> = portfolio
            .positions
            .values()
            .filter(|p| p.qty_raw > 0)
            .cloned()
            .collect();
        let equity_usd = portfolio_equity(&portfolio, &snaps).await;

        let ctx = crate::strategies::traits::EvalContext {
            strategy_id: &req.strategy_id,
            strategy_version: strategy.version(),
            now,
            sol_mint: &req.sol_mint,
            // Aller: replay data is old, never "stale" by wall clock.
            max_price_age_ms: u64::MAX,
            account_cash_lamports: portfolio.cash_lamports,
            equity_usd,
            positions: &positions_vec,
            snapshots: &snaps,
            cache: &replay_cache,
        };
        let proposals = match strategy.evaluate(&cfg, &ctx).await {
            Ok(p) => p,
            Err(e) => {
                return finish_failed(&db, &run_id, &format!("strategy error: {e}")).await;
            }
        };

        for p in proposals {
            signals += 1;
            match apply_replay(
                &mut portfolio,
                &p,
                &replay_cache,
                &req,
                &fee_model,
                now,
            )
            .await
            {
                Ok(Some((closed, fee, slip))) => {
                    if let Some(c) = closed {
                        realized_pnls.push(c);
                    }
                    fee_total_usd += fee;
                    slippage_total_usd += slip;
                }
                Ok(None) => rejections += 1,
                Err(e) => {
                    tracing::warn!(error = %e, "replay apply failed (skipped)");
                    rejections += 1;
                }
            }
        }

        if portfolio.positions.values().any(|p| p.qty_raw > 0) {
            in_market_bars += 1;
        }
        equity_curve.push(portfolio_equity(&portfolio, &snaps).await);
    }

    let end_equity_usd = match equity_curve.last() {
        Some(v) => *v,
        None => lamports_to_usd(req.initial_lamports, start_solusd),
    };
    let start_equity_usd = lamports_to_usd(req.initial_lamports, start_solusd);
    let metrics = compute_metrics(
        start_equity_usd,
        end_equity_usd,
        &equity_curve,
        &realized_pnls,
        signals,
        rejections,
        fee_total_usd,
        slippage_total_usd,
        events.len(),
        in_market_bars,
    );
    let metrics_json = serde_json::to_string(&metrics)
        .map_err(|e| AppError::Internal(format!("metrics serialize: {e}")))?;
    repos::backtests::finish_backtest(&db, &run_id, &metrics, &metrics_json).await?;
    repos::backtests::get_backtest(&db, &run_id).await
}

async fn portfolio_equity(portfolio: &SimPortfolio, snaps: &[MarketSnapshot]) -> Decimal {
    let marks = crate::trading::portfolio::marks_from_snapshots(
        snaps,
        crate::market_data::providers::mock::SOL_MINT,
    );
    let mut value = Decimal::ZERO;
    for p in portfolio.positions.values() {
        if p.qty_raw > 0 {
            if let Some(price) = marks.price_of(&p.mint) {
                value += raw_to_human(p.qty_raw, 9) * price;
            }
        }
    }
    lamports_to_usd(portfolio.cash_lamports, marks.solusd) + value
}

/// Apply one proposal with the same fill conventions as the paper engine.
/// Returns (realized-pnl-usd, fee-usd, slippage-usd) on an executed trade.
#[allow(clippy::too_many_arguments)]
async fn apply_replay(
    portfolio: &mut SimPortfolio,
    p: &SignalProposal,
    cache: &MarketCache,
    req: &BacktestRequest,
    fee_model: &FeeModel,
    _now: i64,
) -> Result<Option<(Option<Decimal>, Decimal, Decimal)>, AppError> {
    let Some(snap) = cache.latest_for(&p.mint).await else {
        return Ok(None);
    };
    let decimals = 9_i32;
    let price = snap.price;
    let sols = cache.latest_for(&req.sol_mint).await;
    let solusd = sols.map(|s| s.price).unwrap_or_else(|| Decimal::from(150));
    let size_usd = Decimal::from(p.proposed_size.unwrap_or(0)) / Decimal::from(100);
    let slip_frac = Decimal::from(req.slippage_bps) / Decimal::from(10_000);

    match p.direction {
        Direction::Buy => {
            let fill_price = price * (Decimal::ONE + slip_frac);
            let qty = human_to_raw(size_usd / fill_price, decimals);
            if qty <= 0 {
                return Ok(None);
            }
            let notional = raw_to_human(qty, decimals) * fill_price;
            let cost = usd_to_lamports(notional, solusd);
            let fee = fee_model.fee_lamports(notional, solusd);
            if cost + fee > portfolio.cash_lamports {
                return Ok(None);
            }
            portfolio.cash_lamports -= cost + fee;
            let entry = portfolio
                .positions
                .entry(p.mint.clone())
                .or_insert_with(|| Position {
                    account_id: "backtest".into(),
                    mint: p.mint.clone(),
                    qty_raw: 0,
                    avg_entry_price: Decimal::ZERO,
                    status: "open".into(),
                    opened_at: 0,
                    updated_at: 0,
                });
            let total = entry.qty_raw + qty;
            if total > 0 {
                entry.avg_entry_price = (raw_to_human(entry.qty_raw, decimals)
                    * entry.avg_entry_price
                    + raw_to_human(qty, decimals) * fill_price)
                    / raw_to_human(total, decimals);
            }
            entry.qty_raw = total;
            let slip_usd = notional * slip_frac / (Decimal::ONE + slip_frac);
            Ok(Some((None, fee_model.fee_usd(notional), slip_usd)))
        }
        Direction::Sell => {
            let Some(pos) = portfolio.positions.get(&p.mint) else {
                return Ok(None);
            };
            if pos.qty_raw <= 0 {
                return Ok(None);
            }
            let fill_price = price * (Decimal::ONE - slip_frac);
            let qty = pos
                .qty_raw
                .min(human_to_raw(size_usd / fill_price, decimals));
            if qty <= 0 {
                return Ok(None);
            }
            let notional = raw_to_human(qty, decimals) * fill_price;
            let proceeds = usd_to_lamports(notional, solusd);
            let fee = fee_model.fee_lamports(notional, solusd);
            if proceeds < fee {
                return Ok(None);
            }
            let realized_usd = notional
                - raw_to_human(qty, decimals) * pos.avg_entry_price
                - fee_model.fee_usd(notional);
            portfolio.cash_lamports += proceeds - fee;
            let pos = portfolio.positions.get_mut(&p.mint).unwrap();
            pos.qty_raw -= qty;
            if pos.qty_raw == 0 {
                pos.avg_entry_price = Decimal::ZERO;
            }
            let slip_usd = notional * slip_frac / (Decimal::ONE - slip_frac);
            Ok(Some((Some(realized_usd), fee_model.fee_usd(notional), slip_usd)))
        }
        Direction::Hold => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_metrics(
    start_equity_usd: Decimal,
    end_equity_usd: Decimal,
    equity_curve: &[Decimal],
    realized: &[Decimal],
    signals: i64,
    rejections: i64,
    fees: Decimal,
    slippage: Decimal,
    total_bars: usize,
    in_market_bars: usize,
) -> BacktestMetrics {
    let net_return = if start_equity_usd > Decimal::ZERO {
        (end_equity_usd - start_equity_usd) / start_equity_usd
    } else {
        Decimal::ZERO
    };
    let max_dd = max_drawdown(equity_curve);
    let (win_rate, avg_win, avg_loss) = crate::analytics::metrics::trade_stats(realized);
    let wins = realized.iter().filter(|p| **p > Decimal::ZERO).count() as i64;
    let losses = realized.iter().filter(|p| **p < Decimal::ZERO).count() as i64;
    let profit_factor = match (avg_win, avg_loss) {
        (Some(w), Some(l)) if w > Decimal::ZERO && l < Decimal::ZERO => Some((w / -l).round_dp(3)),
        (Some(w), None) if w > Decimal::ZERO => Some(Decimal::MAX),
        _ => None,
    };
    BacktestMetrics {
        start_equity_usd,
        end_equity_usd,
        net_return_pct: net_return,
        realized_pnl_usd: realized.iter().sum(),
        unrealized_pnl_usd: end_equity_usd
            - start_equity_usd
            - realized.iter().sum::<Decimal>(),
        max_drawdown_pct: max_dd,
        win_rate_pct: win_rate,
        trade_count: wins + losses,
        wins,
        losses,
        avg_win_usd: avg_win,
        avg_loss_usd: avg_loss,
        profit_factor,
        fee_total_usd: fees,
        slippage_total_usd: slippage,
        time_in_market_pct: if total_bars > 0 {
            Some(Decimal::from(in_market_bars) / Decimal::from(total_bars))
        } else {
            None
        },
        risk_rejections: rejections,
        signals,
        data_gap_percent: Decimal::ZERO,
        qualified: total_bars >= 20 && (wins + losses) >= 1,
        qualification_note: if total_bars >= 20 {
            None
        } else {
            Some(format!(
                "only {total_bars} observations — too few to qualify as a meaningful performance sample"
            ))
        },
    }
}

fn max_drawdown(curve: &[Decimal]) -> Decimal {
    let mut peak = Decimal::ZERO;
    let mut max_dd = Decimal::ZERO;
    for v in curve {
        peak = peak.max(*v);
        if peak > Decimal::ZERO {
            max_dd = max_dd.max((peak - *v) / peak);
        }
    }
    max_dd
}

async fn finish_failed(db: &Database, run_id: &str, error: &str) -> Result<BacktestRun, AppError> {
    repos::backtests::fail_backtest(db, run_id, error).await?;
    repos::backtests::get_backtest(db, run_id).await
}

/// Insert a queued run row, then spawn the worker. Returns the run.
pub async fn enqueue_backtest(
    db: &Database,
    req: BacktestRequest,
) -> Result<BacktestRun, AppError> {
    let run_id = crate::ids::new_id();
    let row = BacktestRun {
        id: run_id.clone(),
        strategy_id: req.strategy_id.clone(),
        strategy_version: 1,
        config_json: req.config_json.clone(),
        data_source: "persisted-snapshots".into(),
        interval_start: req.interval_start,
        interval_end: req.interval_end,
        fee_model: format!("fixed-bps({})", req.fee_bps),
        slippage_bps: Decimal::from(req.slippage_bps),
        status: BacktestStatus::Queued,
        metrics_json: None,
        net_return_pct: None,
        max_drawdown_pct: None,
        win_rate_pct: None,
        trade_count: None,
        profit_factor: None,
        started_at: None,
        finished_at: None,
        error: None,
    };
    repos::backtests::insert_backtest(db, &row).await?;
    let db2 = db.clone();
    let run_id_task = run_id.clone();
    tokio::spawn(async move {
        match run_backtest(db2, run_id_task.clone(), req).await {
            Ok(_) => tracing::info!(run = %run_id_task, "backtest completed"),
            Err(e) => tracing::error!(run = %run_id_task, error = %e, "backtest failed"),
        }
    });
    repos::backtests::get_backtest(db, &run_id).await
}