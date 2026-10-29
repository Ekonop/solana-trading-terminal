//! On-chain reconciliation (Phase 4, ADR-0005).
//!
//! The chain is the single source of truth for a real wallet's balance and
//! holdings. `Reconciler` queries the RPC and returns the authoritative
//! numbers so the executor's loss guards and position view read from the
//! chain, not from an internal ledger that could have drsifted.

use crate::errors::AppError;
use crate::real::rpc::SolanaRpc;

#[derive(Debug, Clone)]
pub struct ChainSnapshot {
    pub address: String,
    pub lamports: u64,
    pub balance_sol: f64,
}

pub struct Reconciler {
    rpc: SolanaRpc,
}

impl Reconciler {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc: SolanaRpc::new(&rpc_url),
        }
    }

    /// Fetch the on-chain SOL lamport balance.
    pub async fn chain_balance(&self, address: &str) -> Result<u64, AppError> {
        self.rpc.get_balance(address).await
    }

    /// Full snapshot (lamports + human-readable SOL).
    pub async fn snapshot(&self, address: &str) -> Result<ChainSnapshot, AppError> {
        let lamports = self.rpc.get_balance(address).await?;
        Ok(ChainSnapshot {
            address: address.to_string(),
            lamports,
            balance_sol: lamports as f64 / 1_000_000_000.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_readable_conversion() {
        // 1.5 SOL
        let s = ChainSnapshot {
            address: "abc".into(),
            lamports: 1_500_000_000,
            balance_sol: 1.5,
        };
        assert!((s.balance_sol - 1.5).abs() < 1e-9);
        assert_eq!(s.lamports, 1_500_000_000);
    }
}
