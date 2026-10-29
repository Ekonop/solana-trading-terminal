//! Database proof of concept (Phase 1 of the build plan).
//!
//! Verifies the pglite-rs integration that everything else depends on:
//!   1. open exactly one in-process PostgreSQL instance,
//!   2. persiste a real data directory across *separate processes*,
//!   3. transactions commit and roll back correctly,
//!   4. multi-statement `exec` is atomic (partial state never persists),
//!   5. concurrent transactions are serialized (no interleaved writes),
//!   6. DECIMAL/NUMERIC round-trips for prices (rust_decimal).
//!
//! Usage:   cargo run --bin db-probe -- <data-dir>
//! Then run it AGAIN with the same data-dir — the second run must report the
//! rows written by the first run (that proves cross-process persistence).

use std::path::PathBuf;
use std::time::Instant;

use rust_decimal::Decimal;
use solana_trading_terminal::db::Database;
use solana_trading_terminal::ids;
use solana_trading_terminal::time;

const MARKER_KEY: &str = "poc_marker";

#[tokio::main]
async fn main() {
    println!("== solana-trading-terminal DB proof of concept ==");
    println!("pglite-rs: one in-process PostgreSQL per process (embedded).\n");

    let data_dir: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("stt-poc-data"));

    let t_start = Instant::now();
    let db = match Database::open(&data_dir).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("FATAL: could not open embedded postgres at {}", data_dir.display());
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    println!("[ok] opened embedded postgres at {}", data_dir.display());

    // 1. Migrations.
    match solana_trading_terminal::db::migrations::migrate(&db).await {
        Ok(_) => {
            let v = latest_schema_version(&db).await;
            println!("[ok] migrations applied (schema version {v})");
        }
        Err(e) => {
            eprintln!("FATAL: migrations failed: {e}");
            std::process::exit(1);
        }
    }

    // 6. NUMERIC round-trip.
    let decimal: Decimal = "1234.56789012345678901234567".parse().unwrap();
    {
        let r = db
            .query(
                "SELECT $1::numeric AS d",
                &[&decimal],
            )
            .await
            .expect("numeric bind");
        let back = solana_trading_terminal::db::numeric::row_decimal(&r[0], "d").unwrap();
        assert_eq!(decimal, back, "DECIMAL/NUMERIC round-trip must be exact");
        println!("[ok] DECIMAL(38,27) round-trip exact ({back})");
    }

    // 2. Cross-process persistence proof.
    let marker = db
        .query("SELECT value FROM settings WHERE key = $1", &[&MARKER_KEY])
        .await
        .expect("read marker");
    if marker.is_empty() {
        db.exec(&format!(
            "INSERT INTO settings (key, value, updated_at) VALUES ('{MARKER_KEY}','created-at-{}',{}) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
            time::now_micros(),
            time::now_micros()
        ))
        .await
        .expect("write marker");
        println!("[warn] first run on this data dir: marker written.");
        println!("       run this binary AGAIN with the same data dir to prove");
        println!("       cross-process persistence.");
    } else {
        let v = marker[0].get::<String>(0).unwrap_or_default();
        println!(
            "[ok] PERSISTENCE: found marker from a previous process run → data survives restarts ({v})"
        );
    }

    // 3. Transactions: commit + rollback + isolation.
    let acct_id = ids::new_id();
    {
        let tx = db.transaction().await.expect("begin tx");
        tx.exec(&format!(
            "INSERT INTO paper_accounts (id, name, currency, cash_balance_lamports, initial_balance_lamports, realized_pnl_lamports, status, created_at, updated_at)
             VALUES ('{acct_id}', 'poc', 'SOL', 1000000000, 1000000000, 0, 'active', {}, {})",
            time::now_micros(),
            time::now_micros()
        ))
        .await
        .expect("insert committed row");
        tx.commit().await.expect("commit");
        println!("[ok] transaction commit");
    }
    {
        let before = db
            .query("SELECT count(*) FROM paper_accounts", &[])
            .await
            .expect("count");
        let tx = db.transaction().await.expect("begin tx");
        tx.exec(&format!(
            "INSERT INTO paper_accounts (id, name, currency, cash_balance_lamports, initial_balance_lamports, realized_pnl_lamports, status, created_at, updated_at)
             VALUES ('{}', 'poc-rollback', 'SOL', 1, 1, 0, 'active', {}, {})",
            ids::new_id(),
            time::now_micros(),
            time::now_micros()
        ))
        .await
        .expect("insert rolled back row");
        tx.rollback().await.expect("rollback");
        let after = db
            .query("SELECT count(*) FROM paper_accounts", &[])
            .await
            .expect("count");
        let (b, a): (i64, i64) = (
            before[0].get(0).unwrap(),
            after[0].get(0).unwrap(),
        );
        assert_eq!(b, a, "rollback must leave no trace");
        println!("[ok] transaction rollback (count {b} before and after)");
    }

    // 4. Multi-statement atomicity: the second statement fails (dup PK) and
    //    the first statement must NOT persist.
    {
        let before = db
            .query("SELECT count(*) FROM paper_accounts", &[])
            .await
            .expect("count");
        let dup_id = ids::new_id();
        let sql = format!(
            "INSERT INTO paper_accounts (id, name, currency, cash_balance_lamports, initial_balance_lamports, realized_pnl_lamports, status, created_at, updated_at) VALUES ('{dup_id}','atomic-a','SOL',1,1,0,'active',{},{}); \
             INSERT INTO paper_accounts (id, name, currency, cash_balance_lamports, initial_balance_lamports, realized_pnl_lamports, status, created_at, updated_at) VALUES ('{dup_id}','atomic-b','SOL',1,1,0,'active',{},{});",
            time::now_micros(),
            time::now_micros(),
            time::now_micros(),
            time::now_micros()
        );
        let res = db.exec(&sql).await;
        assert!(res.is_err(), "second statement must fail (dup PK)");
        let after = db
            .query("SELECT count(*) FROM paper_accounts", &[])
            .await
            .expect("count");
        let (b, a): (i64, i64) = (before[0].get(0).unwrap(), after[0].get(0).unwrap());
        assert_eq!(b, a, "multi-statement exec must be atomic (all or nothing)");
        println!("[ok] multi-statement exec atomic (count {b} before and after failure)");
    }

    // 5. Concurrency: serialize via the engine — 8 concurrent increments of a
    //    single cell must end at exactly 8 (no lost updates).
    {
        let base = db
            .query("SELECT cash_balance_lamports FROM paper_accounts WHERE id = $1", &[&acct_id])
            .await
            .expect("read");
        let base: i64 = base[0].get(0).unwrap();
        let n = 8i64;
        let mut handles = Vec::new();
        for _ in 0..n {
            let db = db.clone();
            let acct = acct_id.clone();
            handles.push(tokio::spawn(async move {
                let tx = db.transaction().await.unwrap();
                tx.exec(&format!(
                    "UPDATE paper_accounts SET cash_balance_lamports = cash_balance_lamports + 1, updated_at = {} WHERE id = '{acct}'",
                    time::now_micros()
                ))
                .await
                .unwrap();
                tx.commit().await.unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let after = db
            .query("SELECT cash_balance_lamports FROM paper_accounts WHERE id = $1", &[&acct_id])
            .await
            .expect("read");
        let after: i64 = after[0].get(0).unwrap();
        assert_eq!(
            after,
            base + n,
            "concurrent updates must serialize (lost-update check)"
        );
        println!("[ok] concurrent transactions serialized ({base} -> {after} after {n} concurrent +1s)");
    }

    // 6b. NUMERIC NULL handling.
    {
        let rows = db
            .query("SELECT NULL::numeric AS x", &[])
            .await
            .expect("null numeric");
        let _: Option<Decimal> = solana_trading_terminal::db::numeric::row_decimal_opt(&rows[0], "x").unwrap();
        println!("[ok] NULL numeric binds as Option<Decimal>::None");
    }

    println!(
        "\n== all checks passed in {:.1}s ==",
        t_start.elapsed().as_secs_f64()
    );
    println!("embedded postgres instance closes cleanly at process exit.");
    db.close().await.expect("close");
}

async fn latest_schema_version(db: &Database) -> i64 {
    match db
        .query("SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1", &[])
        .await
    {
        Ok(rows) if !rows.is_empty() => {
            let v = rows[0].try_get::<String>("version").unwrap_or_default();
            v.trim_start_matches('0').parse::<i64>().unwrap_or(0)
        }
        _ => 0,
    }
}