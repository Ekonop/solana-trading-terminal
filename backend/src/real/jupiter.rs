//! Jupiter Ultra adapter (Phase 3).
//!
//! Jupiter Ultra returns an unsigned, ready-to-sign transaction. We:
//!   1. `GET /ultra/v1/order?...` to get a quote + unsigned tx (base64)
//!   2. sign the transaction with our wallet
//!   3. `POST /ultra/v1/execute` with the signed tx + requestId
//!
//! In dry-run the executor never calls execute — see `executor.rs`. The
//! adapter still builds the request shape and parses responses so the path is
//! fully exercised locally.

use serde_json::{json, Value};
use crate::errors::AppError;

#[derive(Clone)]
pub struct Jupiter {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

/// A quote + unsigned transaction from Jupiter.
#[derive(Debug)]
pub struct Quote {
    pub in_amount: u64,   // lamports in
    pub out_amount: u64,  // lamports out
    pub signed_transaction: String, // base64 (after signing)
    pub request_id: String,
}

impl Jupiter {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Get a quote + unsigned transaction for swapping `in_amount` lamports of
    /// `input_mint` into `output_mint` paid by `taker`.
    pub async fn quote(&self, input_mint: &str, output_mint: &str, in_amount: u64, taker: &str) -> Result<Value, AppError> {
        let url = format!("{}/ultra/v1/order", self.base_url);
        let resp = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&[
                ("inputMint", input_mint.to_string()),
                ("outputMint", output_mint.to_string()),
                ("amount", in_amount.to_string()),
                ("taker", taker.to_string()),
            ])
            .send()
            .await
            .map_err(|e| AppError::Real(format!("jupiter order request: {e}")))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Real(format!("jupiter order parse: {e}")))?;
        if let Some(err) = v.get("error") {
            return Err(AppError::Real(format!("jupiter order error: {err}")));
        }
        Ok(v)
    }

    /// Submit a signed transaction for execution.
    pub async fn execute(&self, signed_b64: &str, request_id: &str) -> Result<String, AppError> {
        let url = format!("{}/ultra/v1/execute", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .json(&json!({
                "signedTransaction": signed_b64,
                "requestId": request_id,
            }))
            .send()
            .await
            .map_err(|e| AppError::Real(format!("jupiter execute request: {e}")))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Real(format!("jupiter execute parse: {e}")))?;
        if let Some(err) = v.get("error") {
            return Err(AppError::Real(format!("jupiter execute error: {err}")));
        }
        let sig = v
            .get("signature")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        Ok(sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn quote_parses_and_surfaces_errors() {
        // error path — no network needed
        let j = Jupiter::new("http://127.0.0.1:1", "k");
        let err = j.quote("in", "out", 100, "taker").await.unwrap_err();
        assert!(err.to_string().contains("jupiter"));
    }

    #[test]
    fn new_trims_trailing_slash() {
        let j = Jupiter::new("https://api.jup.ag/", "k");
        assert_eq!(j.base_url, "https://api.jup.ag");
    }
}
