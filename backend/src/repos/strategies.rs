//! Strategy, strategy-run, and signal repositories.

use crate::db::numeric::*;
use crate::domain::{
    Direction, RunMode, SignalDecision, SignalRecord, StrategyModel, StrategyRun, StrategyState,
};
use crate::errors::AppError;
use crate::repos::{DbLike, Page, PageOut};
use crate::time::now_micros;

fn row_to_strategy(r: &pglite::Row) -> Result<StrategyModel, AppError> {
    let status = row_str(r, "status")?;
    let status = serde_json::from_str::<StrategyState>(&format!("\"{status}\""))
        .map_err(|_| AppError::Database(format!("unknown strategy status {status}")))?;
    Ok(StrategyModel {
        id: row_str(r, "id")?,
        display_name: row_str(r, "display_name")?,
        description: row_str(r, "description")?,
        status,
        enabled: row_bool(r, "enabled")?,
        config_json: row_str(r, "config_json")?,
        config_version: row_i32(r, "config_version")?,
        last_eval_at: row_i64_opt(r, "last_eval_at")?,
        last_error: row_str_opt(r, "last_error")?,
        created_at: row_i64(r, "created_at")?,
        updated_at: row_i64(r, "updated_at")?,
    })
}

fn row_to_run(r: &pglite::Row) -> Result<StrategyRun, AppError> {
    let mode = row_str(r, "mode")?;
    let mode = serde_json::from_str::<RunMode>(&format!("\"{mode}\""))
        .map_err(|_| AppError::Database(format!("unknown run mode {mode}")))?;
    Ok(StrategyRun {
        id: row_str(r, "id")?,
        strategy_id: row_str(r, "strategy_id")?,
        status: row_str(r, "status")?,
        mode,
        config_json: row_str(r, "config_json")?,
        started_at: row_i64(r, "started_at")?,
        ended_at: row_i64_opt(r, "ended_at")?,
        error: row_str_opt(r, "error")?,
    })
}

fn row_to_signal(r: &pglite::Row) -> Result<SignalRecord, AppError> {
    let direction = row_str(r, "direction")?;
    let direction = serde_json::from_str::<Direction>(&format!("\"{direction}\""))
        .map_err(|_| AppError::Database(format!("unknown direction {direction}")))?;
    let decision = row_str(r, "decision")?;
    let decision = serde_json::from_str::<SignalDecision>(&format!("\"{decision}\""))
        .map_err(|_| AppError::Database(format!("unknown decision {decision}")))?;
    Ok(SignalRecord {
        id: row_str(r, "id")?,
        strategy_id: row_str(r, "strategy_id")?,
        strategy_version: row_i32(r, "strategy_version")?,
        mint: row_str(r, "mint")?,
        direction,
        score: row_decimal_opt(r, "score")?,
        reason: row_str(r, "reason")?,
        proposed_size: row_i64_opt(r, "proposed_size")?,
        decision,
        rejection_reason: row_str_opt(r, "rejection_reason")?,
        data_observed_at: row_i64(r, "data_observed_at")?,
        generated_at: row_i64(r, "generated_at")?,
        order_id: row_str_opt(r, "order_id")?,
    })
}

/// Upsert the built-in strategy rows (idempotent registration).
pub async fn register_strategy(
    db: &impl DbLike,
    id: &str,
    display_name: &str,
    description: &str,
    default_config_json: &str,
) -> Result<(), AppError> {
    let now = now_micros();
    db.query(
        "INSERT INTO strategies
            (id, display_name, description, status, enabled, config_json, config_version, created_at, updated_at)
         VALUES ($1,$2,$3,'disabled',FALSE,$4,1,$5,$5)
         ON CONFLICT (id) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            description = EXCLUDED.description
         RETURNING id",
        &[&id, &display_name, &description, &default_config_json, &now],
    )
    .await
    .map(|_| ())
}

pub async fn get_strategy(db: &impl DbLike, id: &str) -> Result<StrategyModel, AppError> {
    let rows = db
        .query("SELECT * FROM strategies WHERE id = $1", &[&id])
        .await?;
    rows.first()
        .map(row_to_strategy)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("strategy {id}")))
}

pub async fn list_strategies(db: &impl DbLike) -> Result<Vec<StrategyModel>, AppError> {
    let rows = db
        .query("SELECT * FROM strategies ORDER BY id", &[])
        .await?;
    rows.iter().map(row_to_strategy).collect()
}

/// Apply a validated config (JSON) and bump the config version.
pub async fn update_strategy_config(
    db: &impl DbLike,
    id: &str,
    config_json: &str,
) -> Result<StrategyModel, AppError> {
    db.query(
        "UPDATE strategies
         SET config_json = $2, config_version = config_version + 1, updated_at = $3
         WHERE id = $1",
        &[&id, &config_json, &now_micros()],
    )
    .await?;
    get_strategy(db, id).await
}

pub async fn set_strategy_status(
    db: &impl DbLike,
    id: &str,
    status: StrategyState,
) -> Result<(), AppError> {
    db.query(
        "UPDATE strategies SET status = $2, updated_at = $3 WHERE id = $1",
        &[&id, &status.as_str(), &now_micros()],
    )
    .await
    .map(|_| ())
}

pub async fn set_strategy_error(
    db: &impl DbLike,
    id: &str,
    error: Option<&str>,
) -> Result<(), AppError> {
    db.query(
        "UPDATE strategies SET last_error = $2, updated_at = $3 WHERE id = $1",
        &[&id, &error.unwrap_or(""), &now_micros()],
    )
    .await
    .map(|_| ())
}

pub async fn set_last_eval(db: &impl DbLike, id: &str, at: i64) -> Result<(), AppError> {
    db.query(
        "UPDATE strategies SET last_eval_at = $2 WHERE id = $1",
        &[&id, &at],
    )
    .await
    .map(|_| ())
}

// ---------------------------------------------------------------------------
// Strategy runs
// ---------------------------------------------------------------------------

pub async fn start_run(
    db: &impl DbLike,
    run: &StrategyRun,
) -> Result<(), AppError> {
    db.query(
        "INSERT INTO strategy_runs
            (id, strategy_id, status, mode, config_json, started_at)
         VALUES ($1,$2,'running',$3,$4,$5)",
        &[
            &run.id,
            &run.strategy_id,
            &mode_str(run.mode),
            &run.config_json,
            &run.started_at,
        ],
    )
    .await
    .map(|_| ())
}

pub async fn end_run(db: &impl DbLike, run_id: &str, status: &str, error: Option<&str>) -> Result<(), AppError> {
    db.query(
        "UPDATE strategy_runs SET status = $2, ended_at = $3, error = $4 WHERE id = $1",
        &[&run_id, &status, &now_micros(), &error.unwrap_or("")],
    )
    .await
    .map(|_| ())
}

pub async fn list_runs(
    db: &impl DbLike,
    strategy_id: Option<&str>,
    page: Page,
) -> Result<PageOut<StrategyRun>, AppError> {
    let owned_sid: Option<String> = strategy_id.map(|s| s.to_string());
    let (sql, params): (String, Vec<&(dyn pglite::ToSql + Sync)>) = match &owned_sid {
        Some(sid) => (
            format!(
                "SELECT * FROM strategy_runs WHERE strategy_id = $1 ORDER BY started_at DESC {}",
                page.sql_bounds()
            ),
            vec![sid as &(dyn pglite::ToSql + Sync)],
        ),
        None => (
            format!(
                "SELECT * FROM strategy_runs ORDER BY started_at DESC {}",
                page.sql_bounds()
            ),
            vec![],
        ),
    };
    let rows = db.query(&sql, &params).await?;
    let total = match &owned_sid {
        Some(sid) => {
            let r = db
                .query(
                    "SELECT count(*)::bigint AS n FROM strategy_runs WHERE strategy_id = $1",
                    &[&sid],
                )
                .await?;
            r.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64
        }
        None => {
            let r = db
                .query("SELECT count(*)::bigint AS n FROM strategy_runs", &[])
                .await?;
            r.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64
        }
    };
    let items = rows.iter().map(row_to_run).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

fn mode_str(m: RunMode) -> &'static str {
    match m {
        RunMode::Live => "live",
        RunMode::Replay => "replay",
        RunMode::Demo => "demo",
    }
}

// ---------------------------------------------------------------------------
// Signals
// ---------------------------------------------------------------------------

/// Persist an evaluated signal (decision attached). Returns the record.
pub async fn insert_signal(
    db: &impl DbLike,
    rec: &SignalRecord,
) -> Result<SignalRecord, AppError> {
    let order_id_sql: Option<&str> = rec.order_id.as_deref(); // NULL, not "" (FK)
    db.query(
        "INSERT INTO strategy_signals
            (id, strategy_id, strategy_version, mint, direction, score, reason,
             proposed_size, decision, rejection_reason, data_observed_at, generated_at, order_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        &[
            &rec.id,
            &rec.strategy_id,
            &rec.strategy_version,
            &rec.mint,
            &direction_str(rec.direction),
            &score_param(&rec.score),
            &rec.reason,
            &rec.proposed_size.unwrap_or(0),
            &decision_str(rec.decision),
            &rec.rejection_reason.as_deref().unwrap_or(""),
            &rec.data_observed_at,
            &rec.generated_at,
            &order_id_sql,
        ],
    )
    .await?;
    Ok(rec.clone())
}

pub async fn set_signal_order(db: &impl DbLike, signal_id: &str, order_id: &str) -> Result<(), AppError> {
    db.query(
        "UPDATE strategy_signals SET order_id = $2 WHERE id = $1",
        &[&signal_id, &order_id],
    )
    .await
    .map(|_| ())
}

pub async fn list_signals(
    db: &impl DbLike,
    strategy_id: Option<&str>,
    page: Page,
) -> Result<PageOut<SignalRecord>, AppError> {
    let owned_sid: Option<String> = strategy_id.map(|s| s.to_string());
    let (sql, params, total_sql): (String, Vec<&(dyn pglite::ToSql + Sync)>, String) =
        match &owned_sid {
            Some(sid) => (
                format!(
                    "SELECT * FROM strategy_signals WHERE strategy_id = $1 ORDER BY generated_at DESC {}",
                    page.sql_bounds()
                ),
                vec![sid as &(dyn pglite::ToSql + Sync)],
                "SELECT count(*)::bigint AS n FROM strategy_signals WHERE strategy_id = $1".into(),
            ),
            None => (
                format!(
                    "SELECT * FROM strategy_signals ORDER BY generated_at DESC {}",
                    page.sql_bounds()
                ),
                vec![],
                "SELECT count(*)::bigint AS n FROM strategy_signals".into(),
            ),
        };
    let rows = db.query(&sql, &params).await?;
    let tr = db.query(&total_sql, &params).await?;
    let total = tr.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0) as u64;
    let items = rows.iter().map(row_to_signal).collect::<Result<_, _>>()?;
    Ok(PageOut::new(items, total, page))
}

pub async fn count_signals(
    db: &impl DbLike,
    strategy_id: &str,
    decision: Option<SignalDecision>,
) -> Result<i64, AppError> {
    let rows = match decision {
        Some(d) => {
            db.query(
                "SELECT count(*)::bigint AS n FROM strategy_signals WHERE strategy_id = $1 AND decision = $2",
                &[&strategy_id, &decision_str(d)],
            )
            .await?
        }
        None => {
            db.query(
                "SELECT count(*)::bigint AS n FROM strategy_signals WHERE strategy_id = $1",
                &[&strategy_id],
            )
            .await?
        }
    };
    Ok(rows.first().map(|r| row_i64(r, "n")).transpose()?.unwrap_or(0))
}

fn score_param(s: &Option<rust_decimal::Decimal>) -> rust_decimal::Decimal {
    s.unwrap_or_else(|| rust_decimal::Decimal::ZERO)
}

pub fn direction_str(d: Direction) -> &'static str {
    match d {
        Direction::Buy => "buy",
        Direction::Sell => "sell",
        Direction::Hold => "hold",
    }
}

pub fn decision_str(d: SignalDecision) -> &'static str {
    match d {
        SignalDecision::Submitted => "submitted",
        SignalDecision::Rejected => "rejected",
    }
}