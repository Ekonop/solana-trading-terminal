//! Risk events, system events, settings, and token metadata repositories.

use crate::db::numeric::*;
use crate::domain::{RiskEvent, SystemEvent, SystemLevel};
use crate::errors::AppError;
use crate::ids;
use crate::repos::{DbLike, Page, PageOut};
use crate::time::now_micros;

fn row_to_risk(r: &pglite::Row) -> Result<RiskEvent, AppError> {
    Ok(RiskEvent {
        id: row_str(r, "id")?,
        ts: row_i64(r, "ts")?,
        kind: row_str(r, "kind")?,
        strategy_id: row_str_opt(r, "strategy_id")?,
        mint: row_str_opt(r, "mint")?,
        order_id: row_str_opt(r, "order_id")?,
        rule: row_str(r, "rule")?,
        detail: row_str_opt(r, "detail")?,
    })
}

fn row_to_system(r: &pglite::Row) -> Result<SystemEvent, AppError> {
    let level = row_str(r, "level")?;
    let level = serde_json::from_str::<SystemLevel>(&format!("\"{level}\""))
        .map_err(|_| AppError::Database(format!("bad level {level}")))?;
    Ok(SystemEvent {
        id: row_str(r, "id")?,
        ts: row_i64(r, "ts")?,
        level,
        subsystem: row_str(r, "subsystem")?,
        message: row_str(r, "message")?,
        correlation_id: row_str_opt(r, "correlation_id")?,
    })
}

pub async fn record_risk_event(db: &impl DbLike, e: &RiskEvent) -> Result<RiskEvent, AppError> {
    db.query(
        "INSERT INTO risk_events (id, ts, kind, strategy_id, mint, order_id, rule, detail)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        &[
            &e.id,
            &e.ts,
            &e.kind,
            &e.strategy_id.as_deref().unwrap_or(""),
            &e.mint.as_deref().unwrap_or(""),
            &e.order_id.as_deref().unwrap_or(""),
            &e.rule,
            &e.detail.as_deref().unwrap_or(""),
        ],
    )
    .await?;
    Ok(e.clone())
}

pub async fn list_risk_events(
    db: &impl DbLike,
    page: Page,
) -> Result<PageOut<RiskEvent>, AppError> {
    let rows = db
        .query(
            &format!("SELECT * FROM risk_events ORDER BY ts DESC {}", page.sql_bounds()),
            &[],
        )
        .await?;
    let tr = db
        .query("SELECT count(*)::bigint AS n FROM risk_events", &[])
        .await?;
    let total = tr.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64;
    let items = rows.iter().map(row_to_risk).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

pub async fn latest_risk_events(db: &impl DbLike, n: usize) -> Result<Vec<RiskEvent>, AppError> {
    let rows = db
        .query(
            &format!("SELECT * FROM risk_events ORDER BY ts DESC LIMIT {}", n.min(100)),
            &[],
        )
        .await?;
    rows.iter().map(row_to_risk).collect()
}

// ---------------------------------------------------------------------------
// System events
// ---------------------------------------------------------------------------

pub async fn record_system_event(
    db: &impl DbLike,
    level: SystemLevel,
    subsystem: &str,
    message: &str,
    correlation_id: Option<&str>,
) -> Result<SystemEvent, AppError> {
    let e = SystemEvent {
        id: ids::new_id(),
        ts: now_micros(),
        level,
        subsystem: subsystem.to_string(),
        message: message.to_string(),
        correlation_id: correlation_id.map(str::to_string),
    };
    let level_str = serde_json::to_string(&e.level)
        .map_err(|_| AppError::Internal("bad level".into()))?
        .trim_matches('"')
        .to_string();
    db.query(
        "INSERT INTO system_events (id, ts, level, subsystem, message, correlation_id)
         VALUES ($1,$2,$3,$4,$5,$6)",
        &[
            &e.id,
            &e.ts,
            &level_str,
            &e.subsystem,
            &e.message,
            &e.correlation_id.as_deref().unwrap_or(""),
        ],
    )
    .await?;
    Ok(e)
}

pub async fn list_system_events(
    db: &impl DbLike,
    page: Page,
) -> Result<PageOut<SystemEvent>, AppError> {
    let rows = db
        .query(
            &format!(
                "SELECT * FROM system_events ORDER BY ts DESC {}",
                page.sql_bounds()
            ),
            &[],
        )
        .await?;
    let tr = db
        .query("SELECT count(*)::bigint AS n FROM system_events", &[])
        .await?;
    let total = tr.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64;
    let items = rows.iter().map(row_to_system).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

// ---------------------------------------------------------------------------
// Settings (persisted, key/value)
// ---------------------------------------------------------------------------

pub async fn get_setting(db: &impl DbLike, key: &str) -> Result<Option<String>, AppError> {
    let rows = db
        .query("SELECT value FROM settings WHERE key = $1", &[&key])
        .await?;
    rows.first()
        .map(|r| row_str(r, "value"))
        .transpose()
}

pub async fn set_setting(db: &impl DbLike, key: &str, value: &str) -> Result<(), AppError> {
    db.query(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1,$2,$3)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
        &[&key, &value, &now_micros()],
    )
    .await
    .map(|_| ())
}

// ---------------------------------------------------------------------------
// Token metadata (untrusted external data — stored raw, used defensively)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TokenMeta {
    pub mint: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub decimals: Option<i32>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

pub async fn upsert_token_meta(
    db: &impl DbLike,
    mint: &str,
    symbol: Option<&str>,
    name: Option<&str>,
    decimals: Option<i32>,
) -> Result<(), AppError> {
    db.query(
        "INSERT INTO token_meta (mint, symbol, name, decimals, first_seen_at, last_seen_at)
         VALUES ($1,$2,$3,$4,$5,$5)
         ON CONFLICT (mint) DO UPDATE SET
            symbol = COALESCE(EXCLUDED.symbol, token_meta.symbol),
            name = COALESCE(EXCLUDED.name, token_meta.name),
            decimals = COALESCE(EXCLUDED.decimals, token_meta.decimals),
            last_seen_at = EXCLUDED.last_seen_at",
        &[&mint, &symbol.unwrap_or(""), &name.unwrap_or(""), &decimals.unwrap_or(-1), &now_micros()],
    )
    .await
    .map(|_| ())
}

pub async fn get_token_meta(db: &impl DbLike, mint: &str) -> Result<Option<TokenMeta>, AppError> {
    let rows = db
        .query("SELECT * FROM token_meta WHERE mint = $1", &[&mint])
        .await?;
    rows.first()
        .map(|r| {
            Ok(TokenMeta {
                mint: row_str(r, "mint")?,
                symbol: row_str_opt(r, "symbol")?,
                name: row_str_opt(r, "name")?,
                decimals: row_i64_opt(r, "decimals")?.map(|v| v as i32),
                first_seen_at: row_i64(r, "first_seen_at")?,
                last_seen_at: row_i64(r, "last_seen_at")?,
            })
        })
        .transpose()
}