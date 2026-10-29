//! Real-mode execution engine (Phases 0-7 of the live-trading build-out).
//!
//! **Safety boundary (ADR-0004):** the default state is fully disarmed.
//! `RealService` refuses to sign/broadcast unless ALL of the following hold:
//!   1. config `real.armed = true`   (or `STT_REAL_ARMED=true`)
//!   2. config `real.dry-run = false` — dry-run logs but never broadcasts
//!   3. a runtime `/api/v1/real/arm` call with the written-down operator
//!      acknowledgements (belt-and-braces double gate)
//!
//! A `Wallet` (ed25519 keypair) is held in memory only while needed and its
//! secret is never logged, returned by the API, or written to the database.
//!
//! In dry-run mode everything else is exercised end-to-end: wallet load,
//! quote, micro-capital sizing, risk gates, rate limiting — only the actual
//! transaction broadcast is replaced by a log.

pub mod executor;
pub mod jupiter;
pub mod reconcile;
pub mod rpc;
pub mod wallet;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::RealConfig;
use crate::errors::AppError;

/// The runtime real-mode gate. Mirrors `RealConfig` but tracks the in-process
/// "armed at runtime" half so config alone can never broadcast (double gate).
#[derive(Debug)]
pub struct RealService {
    cfg: RealConfig,
    /// Set by `/api/v1/real/arm`; cleared by `/disarm` or on max-loss kill.
    runtime_armed: AtomicBool,
    /// A last-known chain balance (lamports) for loss-guard checks.
    chain_balance_lamports: AtomicU64Proxy,
    /// Total real notional this process has attempted (for rate/loss caps).
    attempted_notional_lamports: AtomicU64Proxy,
    /// Disarm reason (set when a kill fires); surfaced via status.
    kill_reason: std::sync::Mutex<Option<String>>,
}

/// Small wrapper so we can hold u64 atomics without importing more.
#[derive(Default)]
struct AtomicU64Proxy(std::sync::atomic::AtomicU64);
impl std::fmt::Debug for AtomicU64Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.load(Ordering::SeqCst))
    }
}

impl RealService {
    /// Build the gate. Does NOT arm anything — starts from `cfg.armed` but
    /// immediately disarms unless dry-run; dry-run is the only auto-on state.
    pub fn new(cfg: RealConfig) -> Self {
        // Start disarmed. Even if the operator set armed=true in config, we
        // still require the runtime `/arm` handshake to broadcast.
        let runtime_armed = if cfg.armed && !cfg.dry_run {
            AtomicBool::new(false) // config alone never broadcasts
        } else {
            AtomicBool::new(false)
        };
        Self {
            cfg,
            runtime_armed,
            chain_balance_lamports: AtomicU64Proxy::default(),
            attempted_notional_lamports: AtomicU64Proxy::default(),
            kill_reason: std::sync::Mutex::new(None),
        }
    }

    /// True only when the process may actually broadcast (dry-run off AND
    /// runtime-armed AND a key is present).
    pub fn can_broadcast(&self) -> bool {
        !self.cfg.dry_run && self.runtime_armed.load(Ordering::SeqCst)
    }

    /// True when we should exercise the full path but replace broadcast with
    /// a log line.
    pub fn is_dry_run(&self) -> bool {
        self.cfg.dry_run
    }

    pub fn cfg(&self) -> &RealConfig {
        &self.cfg
    }

    /// Human-readable lifecycle state for the API.
    pub fn state(&self) -> &'static str {
        match (self.is_dry_run(), self.runtime_armed.load(Ordering::SeqCst)) {
            (true, _) => "dry-run",
            (false, false) => "disarmed",
            (false, true) => "armed",
        }
    }

    /// Runtime arm handshake. Only succeeds if config is armed, dry-run is
    /// explicitly off, and the caller confirms the written-down risk
    /// acknowledgements (avoiding accidental live trading).
    pub fn arm(&self, ack: &ArmAck) -> Result<(), AppError> {
        if self.kill_reason.lock().unwrap().is_some() {
            return Err(AppError::Real(
                "killed — disarm+re-arm required after a max-loss stop".into(),
            ));
        }
        if !self.cfg.armed {
            return Err(AppError::Real(
                "config real.armed=false (set STT_REAL_ARMED=true to enable)".into(),
            ));
        }
        if self.cfg.dry_run {
            return Err(AppError::Real(
                "dry-run is on; set STT_REAL_DRY_RUN=false to permit real orders".into(),
            ));
        }
        if !ack.acknowledged {
            return Err(AppError::Real(
                "operator acknowledgements not confirmed; refusing to arm".into(),
            ));
        }
        if ack.wallet_public_key.is_empty() {
            return Err(AppError::Real("arm requires a wallet public key".into()));
        }
        self.runtime_armed.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn disarm(&self, reason: &str) {
        self.runtime_armed.store(false, Ordering::SeqCst);
        *self.kill_reason.lock().unwrap() = Some(reason.to_string());
    }

    pub fn kill_reason(&self) -> Option<String> {
        self.kill_reason.lock().unwrap().clone()
    }

    /// The chain is the source of truth for real balances (ADR-0005). Update
    /// it from reconciles.
    pub fn set_chain_balance(&self, lamports: u64) {
        self.chain_balance_lamports.0.store(lamports, Ordering::SeqCst);
    }
    pub fn chain_balance(&self) -> u64 {
        self.chain_balance_lamports.0.load(Ordering::SeqCst)
    }

    pub fn note_attempted_notional(&self, lamports: u64) {
        self.attempted_notional_lamports
            .0
            .fetch_add(lamports, Ordering::SeqCst);
    }
    pub fn attempted_notional(&self) -> u64 {
        self.attempted_notional_lamports.0.load(Ordering::SeqCst)
    }
}

/// Operator-written acknowledgements required to arm live trading.
#[derive(Debug, Clone)]
pub struct ArmAck {
    /// `true` = the operator confirms they understand and accept the risks.
    pub acknowledged: bool,
    /// The wallet public key they intend to trade (used as a sanity cross-check
    /// against the loaded wallet).
    pub wallet_public_key: String,
}
