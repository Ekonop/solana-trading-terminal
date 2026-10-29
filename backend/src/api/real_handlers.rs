//! Real-mode REST endpoints (Phase 7).
//!
//! These expose the *status* and the arm/disarm handshake, plus dry-run
//! introspection. They never return wallet secrets. In the default disarmed /
//! dry-run state, every real endpoint is non-destructive.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::error::ApiResult;
use crate::app::AppState;
use crate::config::WalletKeySource;
use crate::real::ArmAck;
use crate::real::wallet::Wallet;
use rust_decimal::prelude::ToPrimitive;

/// Real-mode status (safe to expose).
#[derive(Serialize)]
pub struct RealStatus {
    pub state: &'static str,
    pub dry_run: bool,
    pub configured_armed: bool,
    pub can_broadcast: bool,
    pub wallet_key_source: &'static str,
    pub wallet_address: String,
    pub chain_balance_lamports: u64,
    pub attempted_notional_lamports: u64,
    pub kill_reason: Option<String>,
    pub sol_mint: String,
}

pub async fn real_status(State(st): State<AppState>) -> ApiResult<Json<RealStatus>> {
    let r = st.real;
    // Best-effort wallet address (loads key; never exposes secret).
    let wallet_address = load_address(r.cfg().wallet_key_source).unwrap_or_else(|e| format!("<unavailable: {e}>"));
    // Chain balance read (dry-run last-known; not required to be live).
    let chain_balance = r.chain_balance();
    Ok(Json(RealStatus {
        state: r.state(),
        dry_run: r.is_dry_run(),
        configured_armed: r.cfg().armed,
        can_broadcast: r.can_broadcast(),
        wallet_key_source: match r.cfg().wallet_key_source {
            WalletKeySource::Env => "env",
            WalletKeySource::File => "file",
            WalletKeySource::Stub => "stub",
        },
        wallet_address,
        chain_balance_lamports: chain_balance,
        attempted_notional_lamports: r.attempted_notional(),
        kill_reason: r.kill_reason(),
        sol_mint: r.cfg().sol_mint.clone(),
    }))
}

/// Body for the arm handshake.
#[derive(Deserialize)]
pub struct ArmReq {
    pub acknowledged: bool,
    pub wallet_public_key: String,
}

pub async fn real_arm(
    State(st): State<AppState>,
    Json(req): Json<ArmReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let ack = ArmAck {
        acknowledged: req.acknowledged,
        wallet_public_key: req.wallet_public_key,
    };
    match st.real.arm(&ack) {
        Ok(()) => Ok(Json(json!({ "state": "armed", "dry_run": st.real.is_dry_run() }))),
        Err(e) => Err(e),
    }
}

pub async fn real_disarm(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    st.real.disarm("operator disarm");
    Ok(Json(json!({ "state": "disarmed" })))
}

pub async fn real_wallet(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let r = st.real;
    let addr = load_address(r.cfg().wallet_key_source)?;
    Ok(Json(json!({ "address": addr, "note": "secret never returned" })))
}

pub async fn real_quote_dry(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    // Dry-run sizing preview, no key needed in stub mode.
    let cfg = st.real.cfg().clone();
    let (amount, usd) =
        crate::real::executor::size_order_usd(&cfg, st.real.chain_balance().max(1_000_000_000))?;
    Ok(Json(json!({
        "dry_run": true,
        "sized_input_lamports": amount,
        "intended_usd": usd.to_string(),
        "rent_reserve_lamports": cfg.rent_reserve_lamports,
        "max_order_notional_usd": cfg.max_order_notional_usd.to_string(),
        "micro_capital_usd": cfg.micro_capital_usd.to_string(),
    })))
}

/// Load a wallet address without ever exposing the secret.
fn load_address(source: WalletKeySource) -> Result<String, crate::errors::AppError> {
    let w = Wallet::load(&source, "STT_REAL_WALLET_KEY", "")?;
    Ok(w.address())
}

pub async fn real_sol_price(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    // The single-market micro strategy trades SOL; expose the cached mark.
    let px = st
        .cache
        .sol_price(&st.sol_mint)
        .await
        .and_then(|d| d.to_f64())
        .unwrap_or(150.0);
    Ok(Json(json!({ "sol_usd": px, "note": "from market cache; used for micro sizing" })))
}

/// Execute a real SOL buy through the gated pipeline. Defaults to dry-run:
/// it never broadcasts unless the operator has armed with the written
/// acknowledgements and dry-run is off. Never returns a wallet secret.
pub async fn real_execute_buy(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let r = st.real.clone();
    // Load the wallet from the configured source (secret never leaves this fn).
    let src = r.cfg().wallet_key_source;
    let wallet = Wallet::load(&src, "STT_REAL_WALLET_KEY", &r.cfg().wallet_file)?;
    // Build the executor and run the gated pipeline.
    let exec = crate::real::executor::RealExecutor::new(r.cfg().clone());
    let sol_usd = st
        .cache
        .sol_price(&st.sol_mint)
        .await
        .and_then(|d| d.to_f64())
        .unwrap_or(150.0);
    match exec.execute_buy(&wallet, sol_usd).await {
        Ok(summary) => Ok(Json(json!({ "ok": true, "dry_run": exec.svc.is_dry_run(), "summary": summary }))),
        Err(e) => Err(e),
    }
}
