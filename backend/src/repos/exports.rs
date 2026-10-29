//! Research export helpers: produce CSV of the paper-trading data model.
//!
//! Exports are read-only dumps intended for offline research (pandas,
//! spreadsheets, notebooks). They intentionally mirror the database columns
//! so a researcher can trust the row contract without consulting the API
//! views. All exports are bounded to avoid unbounded memory use: the caller
//! passes a maximum row count (default 100_000), and a `truncated` flag is
//! included for honesty.

use crate::db::numeric::{
    row_bool, row_decimal, row_decimal_opt, row_i32, row_i64, row_i64_opt, row_str, row_str_opt,
};
use crate::errors::AppError;
use crate::repos::DbLike;

pub const EXPORT_LIMIT: i64 = 100_000;

// ---------------------------------------------------------------------------
// CSV escaping
// ---------------------------------------------------------------------------

fn cell(v: impl std::fmt::Display) -> String {
    let s = v.to_string();
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn row_csv(cols: &[String]) -> String {
    cols.join(",") + "\n"
}

fn header(names: &[&str]) -> String {
    row_csv(&names.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

// ---------------------------------------------------------------------------
// Per-resource exporters. Signature: (db, account_id, limit) -> (csv, truncated)
// ---------------------------------------------------------------------------

pub async fn export_fills(
    db: &impl DbLike,
    account_id: &str,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT f.id, f.order_id, f.qty_raw, f.price, f.slippage_bps,
                    f.fee_lamports, f.fee_usd, f.snapshot_ref, f.filled_at, f.is_paper,
                    o.mint, o.side, o.strategy_id
             FROM paper_fills f
             JOIN paper_orders o ON o.id = f.order_id
             WHERE o.account_id = $1
             ORDER BY f.filled_at ASC
             LIMIT $2",
            &[&account_id, &limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "fill_id", "order_id", "mint", "side", "strategy_id", "qty_raw", "price", "slippage_bps",
        "fee_lamports", "fee_usd", "filled_at_us", "is_paper",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "id")?,
            row_str(r, "order_id")?,
            row_str(r, "mint")?,
            row_str(r, "side")?,
            row_str_opt(r, "strategy_id")?.unwrap_or_default(),
            row_i64(r, "qty_raw")?.to_string(),
            row_decimal(r, "price")?.to_string(),
            row_decimal(r, "slippage_bps")?.to_string(),
            row_i64(r, "fee_lamports")?.to_string(),
            row_decimal(r, "fee_usd")?.to_string(),
            row_i64(r, "filled_at")?.to_string(),
            row_bool(r, "is_paper")?.to_string(),
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}

pub async fn export_orders(
    db: &impl DbLike,
    account_id: &str,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT id, strategy_id, mint, side, order_type, qty_raw, limit_price,
                    status, created_at, filled_at, cancel_reason, idempotency_key
             FROM paper_orders
             WHERE account_id = $1
             ORDER BY created_at ASC
             LIMIT $2",
            &[&account_id, &limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "order_id", "strategy_id", "mint", "side", "order_type", "qty_raw", "limit_price",
        "status", "created_at_us", "filled_at_us", "cancel_reason", "idempotency_key",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "id")?,
            row_str_opt(r, "strategy_id")?.unwrap_or_default(),
            row_str(r, "mint")?,
            row_str(r, "side")?,
            row_str(r, "order_type")?,
            row_i64(r, "qty_raw")?.to_string(),
            row_decimal_opt(r, "limit_price")?.map(|d| d.to_string()).unwrap_or_default(),
            row_str(r, "status")?,
            row_i64(r, "created_at")?.to_string(),
            row_i64_opt(r, "filled_at")?.map(|v| v.to_string()).unwrap_or_default(),
            row_str_opt(r, "cancel_reason")?.unwrap_or_default(),
            row_str(r, "idempotency_key")?,
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}

pub async fn export_ledger(
    db: &impl DbLike,
    account_id: &str,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT id, ts, kind, amount_lamports, balance_after, ref_id, correlation_id, note
             FROM ledger_entries
             WHERE account_id = $1
             ORDER BY ts ASC
             LIMIT $2",
            &[&account_id, &limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "entry_id", "ts_us", "kind", "amount_lamports", "balance_after_lamports", "ref_id",
        "correlation_id", "note",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "id")?,
            row_i64(r, "ts")?.to_string(),
            row_str(r, "kind")?,
            row_i64(r, "amount_lamports")?.to_string(),
            row_i64(r, "balance_after")?.to_string(),
            row_str_opt(r, "ref_id")?.unwrap_or_default(),
            row_str_opt(r, "correlation_id")?.unwrap_or_default(),
            row_str_opt(r, "note")?.unwrap_or_default(),
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}

pub async fn export_signals(
    db: &impl DbLike,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT id, strategy_id, strategy_version, mint, direction, score, reason,
                    proposed_size, decision, rejection_reason, data_observed_at, generated_at,
                    order_id
             FROM strategy_signals
             ORDER BY generated_at ASC
             LIMIT $1",
            &[&limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "signal_id", "strategy_id", "strategy_version", "mint", "direction", "score", "reason",
        "proposed_size", "decision", "rejection_reason", "data_observed_at_us", "generated_at_us",
        "order_id",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "id")?,
            row_str(r, "strategy_id")?,
            row_i32(r, "strategy_version")?.to_string(),
            row_str(r, "mint")?,
            row_str(r, "direction")?,
            row_decimal_opt(r, "score")?.map(|d| d.to_string()).unwrap_or_default(),
            cell(row_str(r, "reason")?),
            row_i64_opt(r, "proposed_size")?.map(|v| v.to_string()).unwrap_or_default(),
            row_str(r, "decision")?,
            row_str_opt(r, "rejection_reason")?.unwrap_or_default(),
            row_i64(r, "data_observed_at")?.to_string(),
            row_i64(r, "generated_at")?.to_string(),
            row_str_opt(r, "order_id")?.unwrap_or_default(),
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}

pub async fn export_positions(
    db: &impl DbLike,
    account_id: &str,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT account_id, mint, qty_raw, avg_entry_price, status, opened_at, updated_at
             FROM positions
             WHERE account_id = $1
             ORDER BY opened_at ASC",
            &[&account_id],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "account_id", "mint", "qty_raw", "avg_entry_price", "status", "opened_at_us",
        "updated_at_us",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "account_id")?,
            row_str(r, "mint")?,
            row_i64(r, "qty_raw")?.to_string(),
            row_decimal(r, "avg_entry_price")?.to_string(),
            row_str(r, "status")?,
            row_i64(r, "opened_at")?.to_string(),
            row_i64(r, "updated_at")?.to_string(),
        ]));
    }
    Ok((out, false))
}

pub async fn export_equity(
    db: &impl DbLike,
    account_id: &str,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT ts, cash_lamports, positions_value_usd, equity_usd, unrealized_pnl_usd,
                    realized_pnl_lamports, exposure_pct
             FROM equity_snapshots
             WHERE account_id = $1
             ORDER BY ts ASC
             LIMIT $2",
            &[&account_id, &limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "ts_us", "cash_lamports", "positions_value_usd", "equity_usd", "unrealized_pnl_usd",
        "realized_pnl_lamports", "exposure_pct",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_i64(r, "ts")?.to_string(),
            row_i64(r, "cash_lamports")?.to_string(),
            row_decimal(r, "positions_value_usd")?.to_string(),
            row_decimal(r, "equity_usd")?.to_string(),
            row_decimal(r, "unrealized_pnl_usd")?.to_string(),
            row_i64(r, "realized_pnl_lamports")?.to_string(),
            row_decimal(r, "exposure_pct")?.to_string(),
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}

pub async fn export_backtests(
    db: &impl DbLike,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT id, strategy_id, strategy_version, data_source, interval_start, interval_end,
                    fee_model, slippage_bps, status, net_return_pct, max_drawdown_pct,
                    win_rate_pct, trade_count, profit_factor, started_at, finished_at, error
             FROM backtest_runs
             ORDER BY created_at ASC
             LIMIT $1",
            &[&limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "backtest_id", "strategy_id", "strategy_version", "data_source", "interval_start_us",
        "interval_end_us", "fee_model", "slippage_bps", "status", "net_return_pct",
        "max_drawdown_pct", "win_rate_pct", "trade_count", "profit_factor", "started_at_us",
        "finished_at_us", "error",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "id")?,
            row_str(r, "strategy_id")?,
            row_i32(r, "strategy_version")?.to_string(),
            row_str(r, "data_source")?,
            row_i64(r, "interval_start")?.to_string(),
            row_i64(r, "interval_end")?.to_string(),
            row_str(r, "fee_model")?,
            row_decimal(r, "slippage_bps")?.to_string(),
            row_str(r, "status")?,
            row_decimal_opt(r, "net_return_pct")?.map(|d| d.to_string()).unwrap_or_default(),
            row_decimal_opt(r, "max_drawdown_pct")?.map(|d| d.to_string()).unwrap_or_default(),
            row_decimal_opt(r, "win_rate_pct")?.map(|d| d.to_string()).unwrap_or_default(),
            row_i64_opt(r, "trade_count")?.map(|v| v.to_string()).unwrap_or_default(),
            row_decimal_opt(r, "profit_factor")?.map(|d| d.to_string()).unwrap_or_default(),
            row_i64_opt(r, "started_at")?.map(|v| v.to_string()).unwrap_or_default(),
            row_i64_opt(r, "finished_at")?.map(|v| v.to_string()).unwrap_or_default(),
            cell(row_str_opt(r, "error")?.unwrap_or_default()),
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}

pub async fn export_risk_events(
    db: &impl DbLike,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT id, ts, kind, strategy_id, mint, order_id, rule, detail
             FROM risk_events
             ORDER BY ts ASC
             LIMIT $1",
            &[&limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "event_id", "ts_us", "kind", "strategy_id", "mint", "order_id", "rule", "detail",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "id")?,
            row_i64(r, "ts")?.to_string(),
            row_str(r, "kind")?,
            row_str_opt(r, "strategy_id")?.unwrap_or_default(),
            row_str_opt(r, "mint")?.unwrap_or_default(),
            row_str_opt(r, "order_id")?.unwrap_or_default(),
            row_str(r, "rule")?,
            cell(row_str_opt(r, "detail")?.unwrap_or_default()),
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}

pub async fn export_system_events(
    db: &impl DbLike,
    limit: i64,
) -> Result<(String, bool), AppError> {
    let rows = db
        .query(
            "SELECT id, ts, level, subsystem, message, correlation_id
             FROM system_events
             ORDER BY ts ASC
             LIMIT $1",
            &[&limit],
        )
        .await?;
    let mut out = String::new();
    out.push_str(&header(&[
        "event_id", "ts_us", "level", "subsystem", "message", "correlation_id",
    ]));
    for r in &rows {
        out.push_str(&row_csv(&[
            row_str(r, "id")?,
            row_i64(r, "ts")?.to_string(),
            row_str(r, "level")?,
            row_str(r, "subsystem")?,
            cell(row_str(r, "message")?),
            row_str_opt(r, "correlation_id")?.unwrap_or_default(),
        ]));
    }
    let truncated = rows.len() as i64 == limit;
    Ok((out, truncated))
}