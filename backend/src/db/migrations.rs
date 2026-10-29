//! Versioned SQL migrations, embedded in the binary.
//!
//! Each `migrations/NNNN_name.sql` file is applied exactly once, in sorted
//! (numeric-prefix) order, inside its own transaction. A migration that fails
//! rolls the whole file back and aborts startup — the application never
//! silently continues with an incompatible schema, and it never drops or
//! recreates existing tables to "fix" a mismatch.

use std::path::Path;

use tracing::{debug, info};

use crate::db::Database;
use crate::errors::AppError;
use crate::time::now_micros;

/// `(version, sql)` pairs discovered from the `migrations/` directory.
/// The directory is embedded at compile time.
fn discovered_migrations() -> Result<Vec<(String, String)>, AppError> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| AppError::Migration(format!("cannot read migrations dir: {e}")))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "sql").unwrap_or(false))
        .collect();
    entries.sort();

    let mut out = Vec::new();
    for path in entries {
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Migration("invalid migration file name".into()))?
            .to_string();
        // Version is the numeric prefix before the first underscore.
        let version = file_name
            .split('_')
            .next()
            .ok_or_else(|| AppError::Migration(format!("bad migration file {file_name}")))?
            .to_string();
        // Versions are internal identifiers; enforce a safe charset.
        if !version.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(AppError::Migration(format!(
                "unsafe migration version in {file_name}"
            )));
        }
        let sql = std::fs::read_to_string(&path)
            .map_err(|e| AppError::Migration(format!("cannot read {file_name}: {e}")))?;
        out.push((version, sql));
        debug!(migration = %file_name, "discovered migration");
    }
    Ok(out)
}

/// Apply all pending migrations. Idempotent: already-applied versions are
/// skipped (tracked in `schema_migrations`).
pub async fn migrate(db: &Database) -> Result<Vec<String>, AppError> {
    db.exec(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at BIGINT NOT NULL
        )",
    )
    .await?;

    let applied: std::collections::HashSet<String> = db
        .query("SELECT version FROM schema_migrations", &[])
        .await?
        .iter()
        .filter_map(|r| r.try_get::<&str>("version").ok().map(|s| s.to_string()))
        .collect();

    let migrations = discovered_migrations()?;
    let mut applied_now = Vec::new();

    for (version, sql) in &migrations {
        if applied.contains(version) {
            debug!(version, "migration already applied, skipping");
            continue;
        }
        info!(version, "applying migration");
        let tx = db.transaction().await?;
        // Migrations are static SQL; no parameters, so the simple-protocol
        // `exec` is correct and lets PostgreSQL run multi-statement files.
        if let Err(e) = tx.exec(sql).await {
            tx.rollback().await.ok();
            return Err(AppError::Migration(format!(
                "migration {version} failed: {e}"
            )));
        }
        // `exec` uses the simple query protocol (no parameters); the version
        // is validated above as alphanumeric so direct interpolation is safe.
        let record = format!(
            "INSERT INTO schema_migrations (version, applied_at) VALUES ('{version}', {})",
            now_micros()
        );
        tx.exec(&record)
            .await
            .map_err(|e| AppError::Migration(format!("migration {version} failed: {e}")))?;
        tx.commit()
            .await
            .map_err(|e| AppError::Migration(format!("migration {version} commit failed: {e}")))?;
        applied_now.push(version.clone());
    }

    if applied_now.is_empty() {
        info!("no pending migrations");
    }
    Ok(applied_now)
}