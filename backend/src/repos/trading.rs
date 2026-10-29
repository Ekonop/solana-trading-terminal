//! Orders, fills, positions, and backtest repositories.

use crate::db::numeric::*;
use crate::domain::*;
use crate::errors::AppError;
use crate::repos::{DbLike, Page, PageOut};

fn row_to_order(r: &pglite::Row) -> Result<PaperOrder, AppError> {
    let side = row_str(r, "side")?;
    let side = match side.as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => return Err(AppError::Database(format!("bad side {side}"))),
    };
    let order_type = row_str(r, "order_type")?;
    let order_type = match order_type.as_str() {
        "market" => OrderType::Market,
        "limit" => OrderType::Limit,
        _ => return Err(AppError::Database(format!("bad order type {order_type}"))),
    };
    let status = row_str(r, "status")?;
    let status = match status.as_str() {
        "open" => OrderStatus::Open,
        "filled" => OrderStatus::Filled,
        "cancelled" => OrderStatus::Cancelled,
        "rejected" => OrderStatus::Rejected,
        _ => return Err(AppError::Database(format!("bad order status {status}"))),
    };
    Ok(PaperOrder {
        id: row_str(r, "id")?,
        idempotency_key: row_str(r, "idempotency_key")?,
        account_id: row_str(r, "account_id")?,
        strategy_id: row_str_opt(r, "strategy_id")?,
        mint: row_str(r, "mint")?,
        side,
        order_type,
        qty_raw: row_i64(r, "qty_raw")?,
        limit_price: row_decimal_opt(r, "limit_price")?,
        status,
        created_at: row_i64(r, "created_at")?,
        filled_at: row_i64_opt(r, "filled_at")?,
        cancel_reason: row_str_opt(r, "cancel_reason")?,
    })
}

fn row_to_fill(r: &pglite::Row) -> Result<Fill, AppError> {
    Ok(Fill {
        id: row_str(r, "id")?,
        order_id: row_str(r, "order_id")?,
        qty_raw: row_i64(r, "qty_raw")?,
        price: row_decimal(r, "price")?,
        slippage_bps: row_decimal(r, "slippage_bps")?,
        fee_lamports: row_i64(r, "fee_lamports")?,
        fee_usd: row_decimal(r, "fee_usd")?,
        snapshot_ref: row_str_opt(r, "snapshot_ref")?,
        filled_at: row_i64(r, "filled_at")?,
        is_paper: row_bool(r, "is_paper")?,
    })
}

fn row_to_position(r: &pglite::Row) -> Result<Position, AppError> {
    Ok(Position {
        account_id: row_str(r, "account_id")?,
        mint: row_str(r, "mint")?,
        qty_raw: row_i64(r, "qty_raw")?,
        avg_entry_price: row_decimal(r, "avg_entry_price")?,
        status: row_str(r, "status")?,
        opened_at: row_i64(r, "opened_at")?,
        updated_at: row_i64(r, "updated_at")?,
        decimals: row_i32(r, "decimals").unwrap_or(9),
    })
}

fn row_to_backtest(r: &pglite::Row) -> Result<BacktestRun, AppError> {
    let status = row_str(r, "status")?;
    let status = serde_json::from_str::<BacktestStatus>(&format!("\"{status}\""))
        .map_err(|_| AppError::Database(format!("bad backtest status {status}")))?;
    Ok(BacktestRun {
        id: row_str(r, "id")?,
        strategy_id: row_str(r, "strategy_id")?,
        strategy_version: row_i32(r, "strategy_version")?,
        config_json: row_str(r, "config_json")?,
        data_source: row_str(r, "data_source")?,
        interval_start: row_i64(r, "interval_start")?,
        interval_end: row_i64(r, "interval_end")?,
        fee_model: row_str(r, "fee_model")?,
        slippage_bps: row_decimal(r, "slippage_bps")?,
        status,
        metrics_json: row_str_opt(r, "metrics_json")?,
        net_return_pct: row_decimal_opt(r, "net_return_pct")?,
        max_drawdown_pct: row_decimal_opt(r, "max_drawdown_pct")?,
        win_rate_pct: row_decimal_opt(r, "win_rate_pct")?,
        trade_count: row_i64_opt(r, "trade_count")?,
        profit_factor: row_decimal_opt(r, "profit_factor")?,
        started_at: row_i64_opt(r, "started_at")?,
        finished_at: row_i64_opt(r, "finished_at")?,
        error: row_str_opt(r, "error")?,
    })
}

// ---------------------------------------------------------------------------
// Orders
// ---------------------------------------------------------------------------

pub async fn insert_order(db: &impl DbLike, o: &PaperOrder) -> Result<PaperOrder, AppError> {
    let strategy_sql: Option<&str> = o.strategy_id.as_deref(); // NULL, not "" (FK)
    db.query(
        "INSERT INTO paper_orders
            (id, idempotency_key, account_id, strategy_id, mint, side, order_type,
             qty_raw, limit_price, status, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'open',$10)",
        &[
            &o.id,
            &o.idempotency_key,
            &o.account_id,
            &strategy_sql,
            &o.mint,
            &side_str(o.side),
            &otype_str(o.order_type),
            &o.qty_raw,
            &o.limit_price.unwrap_or(rust_decimal::Decimal::ZERO),
            &o.created_at,
        ],
    )
    .await?;
    Ok(o.clone())
}

pub async fn get_order(db: &impl DbLike, id: &str) -> Result<PaperOrder, AppError> {
    let rows = db
        .query("SELECT * FROM paper_orders WHERE id = $1", &[&id])
        .await?;
    rows.first()
        .map(row_to_order)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("order {id}")))
}

pub async fn get_order_by_idempotency(
    db: &impl DbLike,
    account_id: &str,
    key: &str,
) -> Result<Option<PaperOrder>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM paper_orders WHERE account_id = $1 AND idempotency_key = $2",
            &[&account_id, &key],
        )
        .await?;
    rows.first().map(row_to_order).transpose()
}

pub async fn list_orders(
    db: &impl DbLike,
    account_id: &str,
    status: Option<OrderStatus>,
    page: Page,
) -> Result<PageOut<PaperOrder>, AppError> {
    let owned_status: Option<String> = status.map(|s| status_str(s).to_string());
    let (sql, params) = match &owned_status {
        Some(s) => (
            format!(
                "SELECT * FROM paper_orders WHERE account_id = $1 AND status = $2
                 ORDER BY created_at DESC {}",
                page.sql_bounds()
            ),
            vec![
                &account_id as &(dyn pglite::ToSql + Sync),
                s as &(dyn pglite::ToSql + Sync),
            ],
        ),
        None => (
            format!(
                "SELECT * FROM paper_orders WHERE account_id = $1 ORDER BY created_at DESC {}",
                page.sql_bounds()
            ),
            vec![&account_id as &(dyn pglite::ToSql + Sync)],
        ),
    };
    let rows = db.query(&sql, &params).await?;
    let (total_sql, total_params): (String, Vec<&(dyn pglite::ToSql + Sync)>) = match &owned_status {
        Some(s) => (
            "SELECT count(*)::bigint AS n FROM paper_orders WHERE account_id = $1 AND status = $2"
                .into(),
            vec![
                &account_id as &(dyn pglite::ToSql + Sync),
                s as &(dyn pglite::ToSql + Sync),
            ],
        ),
        None => (
            "SELECT count(*)::bigint AS n FROM paper_orders WHERE account_id = $1".into(),
            vec![&account_id as &(dyn pglite::ToSql + Sync)],
        ),
    };
    let tr = db.query(&total_sql, &total_params).await?;
    let total = tr.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64;
    let items = rows.iter().map(row_to_order).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

/// Open *limit* orders for an account (serviced by the poller).
pub async fn open_limit_orders(
    db: &impl DbLike,
    account_id: &str,
) -> Result<Vec<PaperOrder>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM paper_orders
             WHERE account_id = $1 AND status = 'open' AND order_type = 'limit'
             ORDER BY created_at ASC",
            &[&account_id],
        )
        .await?;
    rows.iter().map(row_to_order).collect()
}

/// Mutate order status (call inside a transaction).
pub async fn set_order_status(
    db: &impl DbLike,
    id: &str,
    status: OrderStatus,
    filled_at: Option<i64>,
    cancel_reason: Option<&str>,
) -> Result<(), AppError> {
    db.query(
        "UPDATE paper_orders SET status = $2, filled_at = $3, cancel_reason = $4 WHERE id = $1",
        &[
            &id,
            &status_str(status),
            &filled_at.unwrap_or(0),
            &cancel_reason.unwrap_or(""),
        ],
    )
    .await
    .map(|_| ())
}

// ---------------------------------------------------------------------------
// Fills
// ---------------------------------------------------------------------------

pub async fn insert_fill(db: &impl DbLike, f: &Fill) -> Result<Fill, AppError> {
    db.query(
        "INSERT INTO paper_fills
            (id, order_id, qty_raw, price, slippage_bps, fee_lamports, fee_usd,
             snapshot_ref, filled_at, is_paper)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,TRUE)",
        &[
            &f.id,
            &f.order_id,
            &f.qty_raw,
            &f.price,
            &f.slippage_bps,
            &f.fee_lamports,
            &f.fee_usd,
            &f.snapshot_ref.as_deref().unwrap_or(""),
            &f.filled_at,
        ],
    )
    .await?;
    Ok(f.clone())
}

pub async fn fills_for_order(db: &impl DbLike, order_id: &str) -> Result<Vec<Fill>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM paper_fills WHERE order_id = $1 ORDER BY filled_at",
            &[&order_id],
        )
        .await?;
    rows.iter().map(row_to_fill).collect()
}

pub async fn list_fills(
    db: &impl DbLike,
    account_id: &str,
    page: Page,
) -> Result<PageOut<Fill>, AppError> {
    let rows = db
        .query(
            &format!(
                "SELECT f.* FROM paper_fills f
                 JOIN paper_orders o ON o.id = f.order_id
                 WHERE o.account_id = $1
                 ORDER BY f.filled_at DESC {}",
                page.sql_bounds()
            ),
            &[&account_id],
        )
        .await?;
    let tr = db
        .query(
            "SELECT count(*)::bigint AS n FROM paper_fills f
             JOIN paper_orders o ON o.id = f.order_id WHERE o.account_id = $1",
            &[&account_id],
        )
        .await?;
    let total = tr.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64;
    let items = rows.iter().map(row_to_fill).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

// ---------------------------------------------------------------------------
// Positions
// ---------------------------------------------------------------------------

/// Read a position inside a transaction (locks the account's row via the
/// transaction's serialization; see `repos::trading` for the account row lock).
pub async fn get_position(
    db: &impl DbLike,
    account_id: &str,
    mint: &str,
) -> Result<Option<Position>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM positions WHERE account_id = $1 AND mint = $2",
            &[&account_id, &mint],
        )
        .await?;
    rows.first().map(row_to_position).transpose()
}

pub async fn list_positions(
    db: &impl DbLike,
    account_id: &str,
) -> Result<Vec<Position>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM positions WHERE account_id = $1 AND status = 'open' ORDER BY opened_at",
            &[&account_id],
        )
        .await?;
    rows.iter().map(row_to_position).collect()
}

/// Upsert the position after an atomic execution (call inside a transaction).
pub async fn upsert_position(
    db: &impl DbLike,
    p: &Position,
) -> Result<(), AppError> {
    db.query(
        "INSERT INTO positions
            (account_id, mint, qty_raw, avg_entry_price, status, opened_at, updated_at, decimals)
         VALUES ($1,$2,$3,$4,'open',$5,$6,$7)
         ON CONFLICT (account_id, mint) DO UPDATE SET
            qty_raw = $3,
            avg_entry_price = $4,
            status = 'open',
            updated_at = $6,
            decimals = $7",
        &[
            &p.account_id,
            &p.mint,
            &p.qty_raw,
            &p.avg_entry_price,
            &p.opened_at,
            &p.updated_at,
            &p.decimals,
        ],
    )
    .await
    .map(|_| ())
}

pub async fn close_position(db: &impl DbLike, account_id: &str, mint: &str) -> Result<(), AppError> {
    db.query(
        "UPDATE positions SET status = 'closed', updated_at = $3
         WHERE account_id = $1 AND mint = $2",
        &[&account_id, &mint, &crate::time::now_micros()],
    )
    .await
    .map(|_| ())
}

// ---------------------------------------------------------------------------
// Backtests
// ---------------------------------------------------------------------------

pub async fn insert_backtest(db: &impl DbLike, b: &BacktestRun) -> Result<(), AppError> {
    db.query(
        "INSERT INTO backtest_runs
            (id, strategy_id, strategy_version, config_json, data_source,
             interval_start, interval_end, fee_model, slippage_bps, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'queued')",
        &[
            &b.id,
            &b.strategy_id,
            &b.strategy_version,
            &b.config_json,
            &b.data_source,
            &b.interval_start,
            &b.interval_end,
            &b.fee_model,
            &b.slippage_bps,
        ],
    )
    .await
    .map(|_| ())
}

pub async fn get_backtest(db: &impl DbLike, id: &str) -> Result<BacktestRun, AppError> {
    let rows = db
        .query("SELECT * FROM backtest_runs WHERE id = $1", &[&id])
        .await?;
    rows.first()
        .map(row_to_backtest)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("backtest {id}")))
}

pub async fn list_backtests(db: &impl DbLike, page: Page) -> Result<PageOut<BacktestRun>, AppError> {
    let rows = db
        .query(
            &format!(
                "SELECT * FROM backtest_runs ORDER BY started_at DESC NULLS LAST {}",
                page.sql_bounds()
            ),
            &[],
        )
        .await?;
    let tr = db
        .query("SELECT count(*)::bigint AS n FROM backtest_runs", &[])
        .await?;
    let total = tr.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64;
    let items = rows.iter().map(row_to_backtest).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

pub async fn set_backtest_running(db: &impl DbLike, id: &str) -> Result<(), AppError> {
    db.query(
        "UPDATE backtest_runs SET status = 'running', started_at = $2 WHERE id = $1",
        &[&id, &crate::time::now_micros()],
    )
    .await
    .map(|_| ())
}

pub async fn finish_backtest(
    db: &impl DbLike,
    id: &str,
    metrics: &BacktestMetrics,
    metrics_json: &str,
) -> Result<(), AppError> {
    db.query(
        "UPDATE backtest_runs SET
            status = 'completed',
            metrics_json = $2,
            net_return_pct = $3,
            max_drawdown_pct = $4,
            win_rate_pct = $5,
            trade_count = $6,
            profit_factor = $7,
            finished_at = $8
         WHERE id = $1",
        &[
            &id,
            &metrics_json,
            &metrics.net_return_pct,
            &metrics.max_drawdown_pct,
            &metrics.win_rate_pct.unwrap_or(rust_decimal::Decimal::ZERO),
            &metrics.trade_count,
            &metrics.profit_factor.unwrap_or(rust_decimal::Decimal::ZERO),
            &crate::time::now_micros(),
        ],
    )
    .await
    .map(|_| ())
}

pub async fn fail_backtest(db: &impl DbLike, id: &str, error: &str) -> Result<(), AppError> {
    db.query(
        "UPDATE backtest_runs SET status = 'failed', error = $2, finished_at = $3 WHERE id = $1",
        &[&id, &error, &crate::time::now_micros()],
    )
    .await
    .map(|_| ())
}

fn side_str(s: OrderSide) -> &'static str {
    match s {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

fn otype_str(t: OrderType) -> &'static str {
    match t {
        OrderType::Market => "market",
        OrderType::Limit => "limit",
    }
}

fn status_str(s: OrderStatus) -> &'static str {
    match s {
        OrderStatus::Open => "open",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Rejected => "rejected",
    }
}