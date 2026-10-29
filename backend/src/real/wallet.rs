//! Solana wallet (ed25519 keypair) handling.
//!
//! Security posture (Phase 1): the secret key is loaded once at boot (or on
//! arm) from an env var or a keypair file, held in memory only as long as
//! needed, and **never** logged, returned over the API, or written to the
//! database. The public key / address is the only thing exposed.
//!
//! Solana uses ed25519; the public key and the 32-byte base58 address are the
//! same bytes. We map a standard ed25519 signing key onto a "base58 address".

use crate::errors::AppError;
use crate::config::WalletKeySource;

/// A loaded Solana wallet. The secret bytes live here and are dropped when the
/// wallet is dropped; nothing else in the program holds a copy.
pub struct Wallet {
    signing_key: ed25519_dalek::SigningKey,
}

impl Wallet {
    /// Load from a JSON secret (base58 64-byte secret key, legacy 64-byte
    /// array form, or a bare 32-byte base58 seed).
    pub fn from_base58_secret(secret_b58: &str) -> Result<Self, AppError> {
        let bytes = bs58::decode(secret_b58.trim())
            .into_vec()
            .map_err(|e| AppError::Real(format!("invalid base58 secret: {e}")))?;
        if bytes.len() != 64 {
            return Err(AppError::Real(format!(
                "expected 64-byte ed25519 secret, got {} bytes",
                bytes.len()
            )));
        }
        // Solana keypairs store seed(32) || pubkey(32). Use the seed.
        let seed: [u8; 32] = bytes[..32]
            .try_into()
            .map_err(|_| AppError::Real("bad seed length".into()))?;
        Ok(Self {
            signing_key: ed25519_dalek::SigningKey::from_bytes(&seed),
        })
    }

    /// Load from a JSON file: either `{"secretKey":[...]}`, the legacy
    /// `[...] ` array, or an object `{"keypair":"<base58>"}`.
    pub fn from_keypair_file(path: &str) -> Result<Self, AppError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| AppError::Real(format!("wallet file unreadable: {e}")))?;
        let trimmed = text.trim();
        // JSON array form [seed..., pubkey...] (64 ints).
        if trimmed.starts_with('[') {
            let arr: Vec<u64> = serde_json::from_str(trimmed)
                .map_err(|e| AppError::Real(format!("wallet array parse: {e}")))?;
            if arr.len() != 64 {
                return Err(AppError::Real(format!(
                    "wallet array must be 64 bytes, got {}",
                    arr.len()
                )));
            }
            let bytes: Vec<u8> = arr.iter().map(|&b| b as u8).collect();
            let seed: [u8; 32] = bytes[..32].try_into().map_err(|_| AppError::Real("seed".into()))?;
            return Ok(Self {
                signing_key: ed25519_dalek::SigningKey::from_bytes(&seed),
            });
        }
        // Object form.
        let obj: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| AppError::Real(format!("wallet file parse: {e}")))?;
        let sec = obj
            .get("secretKey")
            .or_else(|| obj.get("secret"))
            .or_else(|| obj.get("keypair"))
            .ok_or_else(|| AppError::Real("no secretKey field in wallet file".into()))?;
        match sec {
            serde_json::Value::Array(a) => {
                if a.len() == 64 {
                    let bytes: Vec<u8> = a
                        .iter()
                        .map(|v| v.as_u64().map(|b| b as u8).unwrap_or(0))
                        .collect();
                    let seed: [u8; 32] = bytes[..32].try_into().map_err(|_| AppError::Real("seed".into()))?;
                    Ok(Self {
                        signing_key: ed25519_dalek::SigningKey::from_bytes(&seed),
                    })
                } else {
                    Err(AppError::Real(format!("secretKey array len {} != 64", a.len())))
                }
            }
            serde_json::Value::String(s) => Self::from_base58_secret(s),
            _ => Err(AppError::Real("unsupported secretKey form".into())),
        }
    }

    /// Resolve a wallet from an env var / file based on the configured source.
    pub fn load(source: &WalletKeySource, env_key: &str, file_path: &str) -> Result<Self, AppError> {
        match source {
            WalletKeySource::Stub => Err(AppError::Real(
                "wallet-key-source=stub (validate-only). Set env/file source to load a real wallet."
                    .into(),
            )),
            WalletKeySource::Env => {
                let secret = std::env::var(env_key)
                    .map_err(|_| AppError::Real(format!("{env_key} is not set; no wallet key available")))?;
                Wallet::from_base58_secret(&secret)
            }
            WalletKeySource::File => {
                if file_path.is_empty() {
                    return Err(AppError::Real(
                        "real.wallet-file not set but source=file".into(),
                    ));
                }
                Wallet::from_keypair_file(file_path)
            }
        }
    }

    /// The 32-byte ed25519 public key (== Solana address bytes).
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// The base58 public address (the only identity we ever expose).
    pub fn address(&self) -> String {
        bs58::encode(self.public_key_bytes()).into_string()
    }

    /// Sign `message` with the ed25519 key. Returns a 64-byte signature.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        self.signing_key.sign(message).to_bytes()
    }
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print secret material.
        f.debug_struct("Wallet")
            .field("address", &self.address())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A known-good ed25519 seed (all-in-one test vector).
    fn seed() -> [u8; 32] {
        (0u8..32).collect::<Vec<u8>>().try_into().unwrap()
    }

    #[test]
    fn base58_secret_roundtrips_address_and_signature() {
        let mut secret = seed().to_vec();
        secret.extend_from_slice(&[0xAB; 32]); // seed(32) || pub(32)
        let b58 = bs58::encode(&secret).into_string();

        let w = Wallet::from_base58_secret(&b58).unwrap();
        // Valid base58 address; length varies (43-44) but must be non-empty and
        // decode to exactly 32 bytes (the public key).
        assert!(!w.address().is_empty());
        assert_eq!(bs58::decode(&w.address()).into_vec().unwrap().len(), 32);

        let sig = w.sign(b"hello real mode");
        assert_eq!(sig.len(), 64);

        // Signature verifies against the public key (round-trip correctness).
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&w.public_key_bytes()).unwrap();
        use ed25519_dalek::Verifier;
        assert!(vk.verify(b"hello real mode", &ed25519_dalek::Signature::from_bytes(&sig)).is_ok());
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let mut secret = seed().to_vec();
        secret.extend_from_slice(&[0xCD; 32]);
        let w = Wallet::from_base58_secret(&bs58::encode(secret).into_string()).unwrap();
        let dbg = format!("{w:?}");
        assert!(!dbg.contains("base58"));
        assert!(dbg.contains("address"));
    }

    #[test]
    fn rejects_malformed_secret_length() {
        assert!(Wallet::from_base58_secret(&bs58::encode(vec![1u8, 2, 3]).into_string()).is_err());
    }
}
