//! Account, ledger, and equity repositories.

use crate::db::numeric::*;
use crate::domain::{Account, EquitySnapshot, LedgerEntry, LedgerKind};
use crate::errors::AppError;
use crate::repos::{DbLike, Page, PageOut};
use crate::time::now_micros;

fn row_to_account(r: &pglite::Row) -> Result<Account, AppError> {
    Ok(Account {
        id: row_str(r, "id")?,
        name: row_str(r, "name")?,
        currency: row_str(r, "currency")?,
        initial_balance_lamports: row_i64(r, "initial_balance_lamports")?,
        cash_balance_lamports: row_i64(r, "cash_balance_lamports")?,
        realized_pnl_lamports: row_i64(r, "realized_pnl_lamports")?,
        status: row_str(r, "status")?,
        created_at: row_i64(r, "created_at")?,
        updated_at: row_i64(r, "updated_at")?,
    })
}

fn row_to_ledger(r: &pglite::Row) -> Result<LedgerEntry, AppError> {
    let kind = row_str(r, "kind")?;
    let kind = serde_json::from_str::<LedgerKind>(&format!("\"{kind}\""))
        .map_err(|_| AppError::Database(format!("unknown ledger kind {kind}")))?;
    Ok(LedgerEntry {
        id: row_str(r, "id")?,
        account_id: row_str(r, "account_id")?,
        ts: row_i64(r, "ts")?,
        kind,
        amount_lamports: row_i64(r, "amount_lamports")?,
        balance_after: row_i64(r, "balance_after")?,
        ref_id: row_str_opt(r, "ref_id")?,
        correlation_id: row_str_opt(r, "correlation_id")?,
        note: row_str_opt(r, "note")?,
    })
}

fn row_to_equity(r: &pglite::Row) -> Result<EquitySnapshot, AppError> {
    Ok(EquitySnapshot {
        id: row_str(r, "id")?,
        account_id: row_str(r, "account_id")?,
        ts: row_i64(r, "ts")?,
        cash_lamports: row_i64(r, "cash_lamports")?,
        positions_value_usd: row_decimal(r, "positions_value_usd")?,
        equity_usd: row_decimal(r, "equity_usd")?,
        unrealized_pnl_usd: row_decimal(r, "unrealized_pnl_usd")?,
        realized_pnl_lamports: row_i64(r, "realized_pnl_lamports")?,
        exposure_pct: row_decimal(r, "exposure_pct")?,
    })
}

/// Get the default paper account, creating it if the database is fresh.
pub async fn get_or_create_default_account(
    db: &impl DbLike,
    initial_balance_lamports: i64,
) -> Result<Account, AppError> {
    let rows = db
        .query("SELECT * FROM paper_accounts WHERE name = $1 ORDER BY created_at LIMIT 1", &[&"default"])
        .await?;
    if let Some(r) = rows.first() {
        return row_to_account(r);
    }
    let now = now_micros();
    let id = crate::ids::new_id();
    db.query(
        "INSERT INTO paper_accounts
            (id, name, currency, initial_balance_lamports, cash_balance_lamports,
             realized_pnl_lamports, status, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,0,'active',$6,$6)",
        &[
            &id,
            &"default",
            &"SOL",
            &initial_balance_lamports,
            &initial_balance_lamports,
            &now,
        ],
    )
    .await?;
    // Seed one ledger entry recording the initial deposit.
    db.query(
        "INSERT INTO ledger_entries
            (id, account_id, ts, kind, amount_lamports, balance_after, note)
         VALUES ($1,$2,$3,'initial_deposit',$4,$4,$5)",
        &[&crate::ids::new_id(), &id, &now, &initial_balance_lamports, &"demo paper account created"],
    )
    .await?;
    // Re-read the freshly created row (no async recursion — deterministic).
    let rows = db
        .query("SELECT * FROM paper_accounts WHERE name = $1 ORDER BY created_at LIMIT 1", &[&"default"])
        .await?;
    let row = rows
        .first()
        .ok_or_else(|| AppError::Internal("account row vanished after insert".into()))?;
    row_to_account(row)
}

pub async fn get_account(db: &impl DbLike, id: &str) -> Result<Account, AppError> {
    let rows = db
        .query("SELECT * FROM paper_accounts WHERE id = $1", &[&id])
        .await?;
    rows.first()
        .map(row_to_account)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("account {id}")))
}

pub async fn list_accounts(db: &impl DbLike) -> Result<Vec<Account>, AppError> {
    let rows = db
        .query("SELECT * FROM paper_accounts ORDER BY created_at", &[])
        .await?;
    rows.iter().map(row_to_account).collect()
}

// ---------------------------------------------------------------------------
// Ledger
// ---------------------------------------------------------------------------

/// Insert one ledger entry and return it. `balance_after` must be computed by
/// the caller inside the same transaction (never derive from stale reads).
pub async fn insert_ledger_entry(
    db: &impl DbLike,
    entry: &LedgerEntry,
) -> Result<LedgerEntry, AppError> {
    let kind = serde_json::to_string(&entry.kind)
        .map_err(|_| AppError::Internal("bad ledger kind".into()))?
        .trim_matches('"')
        .to_string();
    db.query(
        "INSERT INTO ledger_entries
            (id, account_id, ts, kind, amount_lamports, balance_after, ref_id, correlation_id, note)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        &[
            &entry.id,
            &entry.account_id,
            &entry.ts,
            &kind,
            &entry.amount_lamports,
            &entry.balance_after,
            &entry.ref_id.as_deref().unwrap_or(""),
            &entry.correlation_id.as_deref().unwrap_or(""),
            &entry.note.as_deref().unwrap_or(""),
        ],
    )
    .await?;
    Ok(entry.clone())
}

pub async fn list_ledger(
    db: &impl DbLike,
    account_id: &str,
    page: Page,
) -> Result<PageOut<LedgerEntry>, AppError> {
    let rows = db
        .query(
            &format!(
                "SELECT * FROM ledger_entries WHERE account_id = $1
                 ORDER BY ts DESC, id DESC {}",
                page.sql_bounds()
            ),
            &[&account_id],
        )
        .await?;
    let total = count(db, "ledger_entries", "account_id", account_id).await?;
    let items = rows.iter().map(row_to_ledger).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

// ---------------------------------------------------------------------------
// Equity snapshots
// ---------------------------------------------------------------------------

pub async fn insert_equity_snapshot(
    db: &impl DbLike,
    snap: &EquitySnapshot,
) -> Result<(), AppError> {
    db.query(
        "INSERT INTO equity_snapshots
            (id, account_id, ts, cash_lamports, positions_value_usd, equity_usd,
             unrealized_pnl_usd, realized_pnl_lamports, exposure_pct)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        &[
            &snap.id,
            &snap.account_id,
            &snap.ts,
            &snap.cash_lamports,
            &snap.positions_value_usd,
            &snap.equity_usd,
            &snap.unrealized_pnl_usd,
            &snap.realized_pnl_lamports,
            &snap.exposure_pct,
        ],
    )
    .await
    .map(|_| ())
}

pub async fn latest_equity(db: &impl DbLike, account_id: &str) -> Result<Option<EquitySnapshot>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM equity_snapshots WHERE account_id = $1 ORDER BY ts DESC LIMIT 1",
            &[&account_id],
        )
        .await?;
    rows.first().map(row_to_equity).transpose()
}

pub async fn equity_series(
    db: &impl DbLike,
    account_id: &str,
    limit: usize,
) -> Result<Vec<EquitySnapshot>, AppError> {
    let rows = db
        .query(
            &format!(
                "SELECT * FROM equity_snapshots WHERE account_id = $1
                 ORDER BY ts ASC LIMIT {}",
                limit.clamp(1, 2000)
            ),
            &[&account_id],
        )
        .await?;
    rows.iter().map(row_to_equity).collect()
}

/// Peak equity (USD) for drawdown measurement.
pub async fn peak_equity_usd(db: &impl DbLike, account_id: &str) -> Result<rust_decimal::Decimal, AppError> {
    let rows = db
        .query(
            "SELECT COALESCE(MAX(equity_usd), 0) AS peak FROM equity_snapshots WHERE account_id = $1",
            &[&account_id],
        )
        .await?;
    rows.first()
        .map(|r| row_decimal(r, "peak"))
        .transpose()
        .map(|v| v.unwrap_or(rust_decimal::Decimal::ZERO))
}

/// Equity at the start of the current UTC day (for daily-loss limits).
pub async fn day_start_equity_usd(
    db: &impl DbLike,
    account_id: &str,
) -> Result<Option<rust_decimal::Decimal>, AppError> {
    let today_start_micros = all_day_start_micros();
    let rows = db
        .query(
            "SELECT equity_usd FROM equity_snapshots
             WHERE account_id = $1 AND ts < $2
             ORDER BY ts DESC LIMIT 1",
            &[&account_id, &today_start_micros],
        )
        .await?;
    rows.first().map(|r| row_decimal(r, "equity_usd")).transpose()
}

fn all_day_start_micros() -> i64 {
    let now = chrono::Utc::now();
    let start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    start
        .and_utc()
        .timestamp_micros()
        .saturating_sub(crate::time::MICROS_PER_SEC) // strictly before today
}

async fn count(db: &impl DbLike, table: &str, col: &str, val: &str) -> Result<u64, AppError> {
    let sql = format!("SELECT count(*)::bigint AS n FROM {table} WHERE {col} = $1");
    let rows = db.query(&sql, &[&val]).await?;
    Ok(rows.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64)
}

/// Set the account cash balance, realized pnl, and updated_at atomically
/// (call only inside a transaction).
pub async fn set_balance(
    db: &impl DbLike,
    account_id: &str,
    cash_lamports: i64,
    realized_pnl_lamports: i64,
) -> Result<(), AppError> {
    if cash_lamports < 0 {
        return Err(AppError::InsufficientBalance);
    }
    db.query(
        "UPDATE paper_accounts
         SET cash_balance_lamports = $2, realized_pnl_lamports = $3, updated_at = $4
         WHERE id = $1",
        &[&account_id, &cash_lamports, &realized_pnl_lamports, &now_micros()],
    )
    .await
    .map(|_| ())
}