//! Server-side secret access (security best practice).
//!
//! **Rule (Helius & Jupiter docs, ADR-0004):** API keys and wallet secrets are
//! resolved entirely server-side, from the process environment or a
//! gitignored `.env` next to the binary. They are:
//!   - never shipped to the client,
//!   - never (re)exposed by any API endpoint,
//!   - never printed in full to logs,
//!   - never read from the committed `stt.toml`.
//!
//! This module is the single place that reads secret material, so it can
//! apply consistent validation + redaction in one spot.

use std::env;
use std::path::PathBuf;

use crate::errors::AppError;

/// A secret value that redacts itself in `Debug`/`Display` (only the first
/// char and length are shown) so accidental logging never leaks the key.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.redacted().fmt(f)
    }
}
impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.redacted().fmt(f)
    }
}
impl Secret {
    fn redacted(&self) -> String {
        let n = self.0.len();
        if n == 0 {
            "<empty secret>".to_string()
        } else {
            let shown = &self.0[..1.max(n.min(4))];
            format!("{shown}…{} chars (redacted)", self.0.chars().count())
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Load a secret from an env var, or from a `.env` file if `load_dotenv` was
/// requested and the env var is unset. `.env` keys are served by dotenvy; this
/// gives us one place to know a key is sourced.
fn get_secret(var: &str) -> Result<Option<Secret>, AppError> {
    match env::var(var) {
        Ok(v) if !v.trim().is_empty() => Ok(Some(Secret(v))),
        Ok(_) => Ok(None),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(AppError::Real(format!("cannot read {var}: {e}"))),
    }
}

/// Resolve the three configured secrets. Returns None for any that aren't set
/// so callers can degrade gracefully (e.g. dry-run, or refuse to arm).
#[derive(Debug, Clone, Default)]
pub struct ResolvedSecrets {
    pub rpc_url: Option<String>,
    pub jupiter_api_key: Option<Secret>,
    pub wallet_key: Option<Secret>,
}

impl ResolvedSecrets {
    /// Load all secrets. `config_rpc_url` is the configured fallback for the
    /// RPC URL (an endpoint URL is a low-sensitivity value; keys are not).
    pub fn load(config_rpc_url: &str) -> Result<Self, AppError> {
        let rpc = env::var("STT_REAL_RPC_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| Some(config_rpc_url.to_string()));
        Ok(Self {
            rpc_url: rpc,
            jupiter_api_key: get_secret("STT_REAL_JUPITER_API_KEY")?,
            wallet_key: get_secret("STT_REAL_WALLET_KEY")?,
        })
    }

    /// True if the wallet secret is present (needed to go live).
    pub fn has_wallet(&self) -> bool {
        self.wallet_key.is_some()
    }
}

/// Attempt to load a `.env` next to this binary / from the CWD. Returns the
/// path that was found, or Ok(None) if none exists (secrets may simply come
/// from the process environment in that case).
pub fn load_dotenv_if_present() -> Result<Option<PathBuf>, AppError> {
    let cwd = env::current_dir().map_err(|e| AppError::Real(format!("cwd: {e}")))?;
    let candidates = [
        cwd.join(".env"),
        cwd.join("secrets.env"),
    ];
    for p in candidates {
        if p.is_file() {
            // Use dotenvy's from_path to load without clobbering existing env.
            dotenvy::from_path(&p)
                .map_err(|e| AppError::Real(format!("reading {}: {e}", p.display())))?;
            return Ok(Some(p));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_redacts_in_display() {
        let s = Secret("abcdef1234567890".to_string());
        let d = format!("{s}");
        assert!(!d.contains("abcdef1234567890"));
        assert!(d.contains("16 chars"));
        assert!(d.contains("redacted"));
    }

    #[test]
    fn empty_secret_redacts_clearly() {
        let s = Secret(String::new());
        assert!(format!("{s}").contains("empty"));
    }
}
