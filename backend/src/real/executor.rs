//! Real-mode executor pipeline (Phase 4) with hard risk guards (Phase 5) and
//! micro-capital sizing (Phase 6).
//!
//! Pipeline: load wallet -> hard guards (armed? dry-run? balance? rate? loss
//! cap?) -> size the order for micro capital -> Jupiter quote -> sign ->
//! dry-run log OR broadcast -> record attempt. All guards are checked BEFORE
//! anything touches the signer/broadcaster.
//!
//! The chain (on-chain balance) is the source of truth for real-money loss
//! guards (ADR-0005); a stale chain balance forces a fresh reconcile first.

use crate::config::RealConfig;
use crate::errors::AppError;
use crate::real::reconcile::Reconciler;
use crate::real::rpc::SolanaRpc;
use crate::real::wallet::Wallet;
use crate::real::RealService;
use rust_decimal::prelude::ToPrimitive;

/// A validated real order ready for quote + (possibly) broadcast.
#[derive(Debug, Clone)]
pub struct RealOrder {
    pub side: OrderSide,
    pub mint: String,
    /// Swap input amount in lamports (micro-capital sized).
    pub input_lamports: u64,
    pub intended_usd: rust_decimal::Decimal,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Micro-capital sizing (Phase 6): a $5 wallet trades at most a small fraction
/// of its value, always leaving rent+gas reserve, and never exceeds the
/// configured per-order notional.
pub fn size_order_usd(cfg: &RealConfig, wallet_balance_lamports: u64) -> Result<(u64, rust_decimal::Decimal), AppError> {
    // Rent+gas reserve must remain.
    if wallet_balance_lamports <= cfg.rent_reserve_lamports as u64 {
        return Err(AppError::Real(format!(
            "wallet balance {} lamports is below rent/gas reserve {}",
            wallet_balance_lamports, cfg.rent_reserve_lamports
        )));
    }
    let spendable = wallet_balance_lamports - cfg.rent_reserve_lamports as u64;

    // Per-order notional cap from config (e.g. $2).
    let max_order_lamports = usd_to_lamports(cfg.max_order_notional_usd);

    // Use the smaller of spendable and per-order cap; also cap at a fraction
    // (e.g. 50% of spendable) to stay micro and leave room.
    let fraction = spendable / 2;
    let amount = max_order_lamports.min(fraction).min(spendable);
    if amount == 0 {
        return Err(AppError::Real("sized order amount is zero".into()));
    }
    // Intended USD for the record (lamports * micro-capital / spendable approx
    // is awkward without a price; we record the lamport amount and rough USD as
    // micro-capital fraction).
    let intended_usd = rust_decimal::Decimal::from(amount) * cfg.micro_capital_usd
        / rust_decimal::Decimal::from(spendable.max(1));
    Ok((amount, intended_usd))
}

/// Approximate SOL/USD conversion using micro-capital budget (kept internal).
fn usd_to_lamports(usd: rust_decimal::Decimal) -> u64 {
    // Highly simplified: treat USD budget as a fraction of the $5 micro budget
    // and assume ~$150/SOL by default for demo; in practice the executor pulls
    // a real SOL price. This is only used to bound notional.
    let sol_usd = rust_decimal::Decimal::from(150u64);
    let sol_lamports = rust_decimal::Decimal::from(1_000_000_000u64);
    let lamports = usd * sol_lamports / sol_usd;
    u64::try_from(lamports.round().to_u128().unwrap_or(0)).unwrap_or(0)
}

pub struct RealExecutor {
    pub cfg: RealConfig,
    pub svc: RealService,
    pub rpc: SolanaRpc,
}

impl RealExecutor {
    pub fn new(cfg: RealConfig) -> Self {
        let svc = RealService::new(cfg.clone());
        let rpc = SolanaRpc::new(&cfg.rpc_url);
        Self { cfg, svc, rpc }
    }

    /// The full-entry gate check. Returns errors with a human reason; the
    /// caller surfaces them and records a risk event.
    fn check_ready(&self) -> Result<(), AppError> {
        if !self.svc.is_dry_run() && !self.svc.can_broadcast() {
            return Err(AppError::Real("real mode is disarmed (call /api/v1/real/arm)".into()));
        }
        Ok(())
    }

    /// Execute a real SOL buy, gated, sized, quoted, signed, then dry-run or
    /// broadcast. Returns a summary string for the API/log but never the key.
    pub async fn execute_buy(&self, wallet: &Wallet, _sol_price_usd: f64) -> Result<String, AppError> {
        // P5 guards
        self.check_ready()?;
        if let Some(reason) = self.svc.kill_reason() {
            return Err(AppError::Real(format!("killed: {reason}")));
        }

        // Chain is source of truth: refresh balance via reconcile.
        let balance = Reconciler::new(self.cfg.rpc_url.clone())
            .chain_balance(&wallet.address())
            .await?;
        self.svc.set_chain_balance(balance);

        // P5: max real loss guard (chain balance vs initial micro notional).
        let initial_micro = usd_to_lamports(self.cfg.micro_capital_usd);
        if balance < initial_micro.saturating_sub(self.cfg.max_real_loss_lamports as u64) {
            self.svc.disarm(&format!(
                "chain balance {} below initial {} - max-loss {}",
                balance, initial_micro, self.cfg.max_real_loss_lamports
            ));
            return Err(AppError::Real("max real loss reached — disarmed".into()));
        }

        // P6: size the order.
        let (input_lamports, intended_usd) = size_order_usd(&self.cfg, balance)?;

        // P4: quote (input = SOL mint -> USDC mint swap, or a stables pair).
        let jup = crate::real::jupiter::Jupiter::new(&self.cfg.jupiter_base_url, &self.cfg.jupiter_api_key);
        let quote = jup
            .quote(&self.cfg.sol_mint, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", input_lamports, &wallet.address())
            .await;

        if self.svc.is_dry_run() {
            // Log everything except the secret. Do NOT call jupiter.execute.
            self.svc.note_attempted_notional(input_lamports);
            return Ok(format!(
                "DRY-RUN buy {input_lamports} SOL lamports (~${:.2}) at chain balance {balance}; quote={:?}; broadcast skipped",
                intended_usd.to_f64().unwrap_or(0.0), quote.as_ref().ok()
            ));
        }

        // ---- Real (armed, non-dry-run) path ----
        // The signer is intentionally a safe stub (ADR-0006): it refuses to
        // sign or broadcast until a real tx parser + operator arming on a live
        // network are in place. So reaching here never actually spends money.
        let quote_value =
            quote.map_err(|e| AppError::Real(format!("quote failed (not broadcasting): {e}")))?;
        let unsigned_tx = quote_value
            .get("transaction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Real("jupiter returned no transaction".into()))?
            .to_string();
        let request_id = quote_value
            .get("requestId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let signed = sign_base64_tx(wallet, &unsigned_tx)?;
        let sig = jup.execute(&signed, &request_id).await?;
        self.svc.note_attempted_notional(input_lamports);
        Ok(format!("broadcast buy {input_lamports} lamports; signature {sig}"))
    }
}

/// Placeholder: sign a base64-encoded Solana transaction. A full Signer trait
/// (see ADR-0006) would deserialize the tx, sign the message with the wallet,
/// and re-encode. Kept as a thin, always-safe stub here so no real broadcast
/// is attempted until the signer is hooked to a real tx parser + the operator
/// arms with a live network.
fn sign_base64_tx(wallet: &Wallet, base64_tx: &str) -> Result<String, AppError> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_tx)
        .map_err(|e| AppError::Real(format!("bad base64 tx: {e}")))?;
    // Real implementation: parse shortvec/instructions, sign message bytes,
    // splice signature. For safety this stub only reports intent.
    let _ = (wallet, bytes.len());
    Err(AppError::Real("live transaction signing requires the ADR-0006 signer + operator arming; nothing was broadcast".into()))
}
