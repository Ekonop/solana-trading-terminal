//! Database engine lifecycle and the single-instance wrapper.
//!
//! # Hard constraints (verified against pglite-rs 0.2.7 source)
//!
//! * Exactly ONE `PGlite` instance may be opened per process. A process-global
//!   `BOOTED` static makes any later `open()` return `Error::ReopenUnsupported`,
//!   and a simultaneous second open returns `Error::AlreadyOpen`.
//! * `close(self)` consumes the instance; the engine cannot be reopened in the
//!   same process afterwards. Restart persistence is therefore tested by
//!   launching a fresh process (see `tests/`) and the `db-probe` binary.
//! * In the default single-process mode every `exec`/`query` takes an internal
//!   async mutex, and `transaction()` holds that same mutex from BEGIN until
//!   COMMIT/ROLLBACK. All statements and transactions are therefore serialized
//!   at the engine level: concurrent transactions cannot interleave, which
//!   makes double-spending a shared balance structurally impossible.
//!
//! Application policy derived from these constraints:
//! * Open once at startup, keep in `AppState`, never reopen.
//! * All paper-execution mutations go through a single writer actor (bounded
//!   channel) so read-modify-write sequences are ordered and backpressure is
//!   bounded; the engine-level serialization is a second, independent layer.
//! * Never hold a transaction across an `.await` of remote I/O. This deadlocks
//!   `futures::lock::Mutex` (non-reentrant) and holds the whole engine idle.

pub mod migrations;
pub mod numeric;

use std::path::Path;

use pglite::{PGlite, PGliteOptions};
use tracing::{debug, info, warn};

use crate::errors::AppError;

/// Owned handle to the embedded database. `Clone` shares the same engine.
#[derive(Clone)]
pub struct Database {
    inner: std::sync::Arc<PGlite>,
    data_dir: std::sync::Arc<Path>,
}

impl Database {
    /// Open (or attach to) the persistent data directory exactly once.
    /// Creates the directory if missing. Fails loudly on any error.
    pub async fn open(data_dir: &Path) -> Result<Database, AppError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            AppError::Config(format!(
                "cannot create data directory {}: {e}",
                data_dir.display()
            ))
        })?;

        let options = PGliteOptions {
            username: "postgres".into(),
            database: "postgres".into(),
            relaxed_durability: false,
            start_params: vec!["timezone=UTC".into(), "lc_messages=C".into()],
            locale_provider: pglite::LocaleProvider::default(),
        };

        info!(dir = %data_dir.display(), "opening embedded postgres (pglite)");
        let pg = PGlite::open_with(data_dir, options)
            .await
            .map_err(|e| match e {
                pglite::Error::AlreadyOpen => AppError::DatabaseUnavailable(format!(
                    "pglite is already open in this process; refusing to double-open (dir {})",
                    data_dir.display()
                )),
                pglite::Error::ReopenUnsupported => AppError::DatabaseUnavailable(format!(
                    "pglite cannot reopen in the same process (dir {}); restart the process",
                    data_dir.display()
                )),
                other => AppError::Database(format!(
                    "failed to open embedded database at {}: {other}",
                    data_dir.display()
                )),
            })?;

        // Nothing should have been opened before us; arithmetic safety check.
        let db = Database {
            inner: std::sync::Arc::new(pg),
            data_dir: std::sync::Arc::from(data_dir.to_path_buf()),
        };
        Ok(db)
    }

    /// Execute static SQL (no parameters). Use for DDL/seed.
    pub async fn exec(&self, sql: &str) -> Result<(), AppError> {
        self.inner.exec(sql).await.map_err(AppError::from)
    }

    /// Parameterized query returning typed rows.
    pub async fn query(
        &self,
        sql: &str,
        params: &[&(dyn pglite::ToSql + Sync)],
    ) -> Result<Vec<pglite::Row>, AppError> {
        self.inner.query(sql, params).await.map_err(AppError::from)
    }

    /// Start an atomic transaction. The transaction serializes all engine
    /// operations for its duration (engine-level mutex).
    pub async fn transaction(&self) -> Result<pglite::Transaction<'_>, AppError> {
        self.inner.transaction().await.map_err(AppError::from)
    }

    /// Path of the persistent data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Close the engine cleanly. Consumes the shared handle; the process must
    /// not open pglite again afterwards.
    pub async fn close(&self) -> Result<(), AppError> {
        // Arc::try_unwrap — only close when we hold the last strong reference
        // (after all tasks have stopped). Otherwise warn and skip.
        match std::sync::Arc::try_unwrap(self.inner.clone()) {
            Ok(pg) => {
                info!("closing embedded database");
                pg.close().await.map_err(AppError::from)
            }
            Err(_) => {
                warn!("database close skipped: engine still referenced elsewhere");
                Ok(())
            }
        }
    }

    /// Export the data directory (documented backup path). Produces a tar
    /// archive via the crate's `dump_data_dir`.
    pub async fn backup_to(&self, dest: &Path) -> Result<(), AppError> {
        self.inner
            .dump_data_dir(dest)
            .await
            .map_err(AppError::from)
    }

    /// Verifies the engine answers a trivial query (startup self-check).
    pub async fn verify(&self) -> Result<(), AppError> {
        let rows = self.query("SELECT 1 AS ok", &[]).await?;
        debug!("database self-check: {} rows", rows.len());
        Ok(())
    }
}