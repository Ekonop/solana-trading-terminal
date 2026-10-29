//! Market snapshot and provider-health repositories.

use crate::db::numeric::*;
use crate::domain::MarketSnapshot;
use crate::errors::AppError;
use crate::repos::{DbLike, Page, PageOut};

fn row_to_snapshot(r: &pglite::Row) -> Result<MarketSnapshot, AppError> {
    Ok(MarketSnapshot {
        id: row_str(r, "id")?,
        mint: row_str(r, "mint")?,
        symbol: row_str_opt(r, "symbol")?,
        name: row_str_opt(r, "name")?,
        pair_id: row_str_opt(r, "pair_id")?,
        price: row_decimal(r, "price")?,
        liquidity_usd: row_decimal_opt(r, "liquidity_usd")?,
        volume_24h_usd: row_decimal_opt(r, "volume_24h_usd")?,
        source: row_str(r, "source")?,
        is_simulated: row_bool(r, "is_simulated")?,
        observed_at: row_i64(r, "observed_at")?,
        received_at: row_i64(r, "received_at")?,
        quality: row_i32(r, "quality")?,
    })
}

pub async fn insert_snapshot(db: &impl DbLike, s: &MarketSnapshot) -> Result<(), AppError> {
    db.query(
        "INSERT INTO market_snapshots
            (id, mint, symbol, name, pair_id, price, liquidity_usd, volume_24h_usd,
             source, is_simulated, observed_at, received_at, quality)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        &[
            &s.id,
            &s.mint,
            &s.symbol.as_deref().unwrap_or(""),
            &s.name.as_deref().unwrap_or(""),
            &s.pair_id.as_deref().unwrap_or(""),
            &s.price,
            &s.liquidity_usd.unwrap_or(rust_decimal::Decimal::ZERO),
            &s.volume_24h_usd.unwrap_or(rust_decimal::Decimal::ZERO),
            &s.source,
            &s.is_simulated,
            &s.observed_at,
            &s.received_at,
            &s.quality,
        ],
    )
    .await
    .map(|_| ())
}

/// Latest snapshot per mint (distinct mints).
pub async fn latest_snapshots(db: &impl DbLike) -> Result<Vec<MarketSnapshot>, AppError> {
    let rows = db
        .query(
            "SELECT s.* FROM market_snapshots s
             JOIN (SELECT mint, MAX(observed_at) AS m FROM market_snapshots GROUP BY mint) l
               ON l.mint = s.mint AND l.m = s.observed_at
             ORDER BY s.observed_at DESC",
            &[],
        )
        .await?;
    rows.iter().map(row_to_snapshot).collect()
}

pub async fn latest_for(db: &impl DbLike, mint: &str) -> Result<Option<MarketSnapshot>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM market_snapshots WHERE mint = $1 ORDER BY observed_at DESC LIMIT 1",
            &[&mint],
        )
        .await?;
    rows.first().map(row_to_snapshot).transpose()
}

pub async fn list_snapshots(
    db: &impl DbLike,
    mint: Option<&str>,
    page: Page,
) -> Result<PageOut<MarketSnapshot>, AppError> {
    let owned_mint: Option<String> = mint.map(|s| s.to_string());
    let (sql, params) = match &owned_mint {
        Some(m) => (
            format!(
                "SELECT * FROM market_snapshots WHERE mint = $1 ORDER BY observed_at DESC {}",
                page.sql_bounds()
            ),
            vec![m as &(dyn pglite::ToSql + Sync)],
        ),
        None => (
            format!(
                "SELECT * FROM market_snapshots ORDER BY observed_at DESC {}",
                page.sql_bounds()
            ),
            vec![],
        ),
    };
    let rows = db.query(&sql, &params).await?;
    let total_sql = match &owned_mint {
        Some(_) => "SELECT count(*)::bigint AS n FROM market_snapshots WHERE mint = $1".to_string(),
        None => "SELECT count(*)::bigint AS n FROM market_snapshots".to_string(),
    };
    let tr = db.query(&total_sql, &params).await?;
    let total = tr.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64;
    let items = rows.iter().map(row_to_snapshot).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

/// Observations for a mint inside [start, end] ascending (backtest/replay).
pub async fn snapshots_in_range(
    db: &impl DbLike,
    mint: &str,
    start: i64,
    end: i64,
) -> Result<Vec<MarketSnapshot>, AppError> {
    let rows = db
        .query(
            "SELECT * FROM market_snapshots
             WHERE mint = $1 AND observed_at >= $2 AND observed_at <= $3
             ORDER BY observed_at ASC",
            &[&mint, &start, &end],
        )
        .await?;
    rows.iter().map(row_to_snapshot).collect()
}

/// Distinct mints with snapshot counts (for data-gap / availability checks).
pub async fn mint_coverage(db: &impl DbLike) -> Result<Vec<(String, i64, i64, i64)>, AppError> {
    let rows = db
        .query(
            "SELECT mint, count(*)::bigint AS n, MIN(observed_at) AS lo, MAX(observed_at) AS hi
             FROM market_snapshots GROUP BY mint ORDER BY n DESC",
            &[],
        )
        .await?;
    rows.iter()
        .map(|r| {
            Ok((
                row_str(r, "mint")?,
                row_i64(r, "n")?,
                row_i64(r, "lo")?,
                row_i64(r, "hi")?,
            ))
        })
        .collect()
}