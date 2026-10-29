//! Minimal Solana JSON-RPC client (Phase 2). We only need a few methods to
//! check balance, get a fresh blockhash, and look up a signature status. Kept
//! deliberately small; a production client would use the full solana-client.

use serde_json::{json, Value};
use crate::errors::AppError;

#[derive(Clone)]
pub struct SolanaRpc {
    url: String,
    client: reqwest::Client,
}

impl SolanaRpc {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, AppError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Real(format!("rpc call {method} failed: {e}")))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Real(format!("rpc {method} bad response: {e}")))?;
        if let Some(err) = v.get("error") {
            return Err(AppError::Real(format!(
                "rpc {method} error: {err}"
            )));
        }
        v.get("result")
            .cloned()
            .ok_or_else(|| AppError::Real(format!("rpc {method} missing result")))
    }

    /// Raw lamport balance of a base58 address.
    pub async fn get_balance(&self, address: &str) -> Result<u64, AppError> {
        let r = self.call("getBalance", json!([address])).await?;
        let lamports = r
            .get("value")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| AppError::Real("getBalance missing value".into()))?;
        Ok(lamports)
    }

    /// Latest blockhash, base58 (needed to build a fresh transaction).
    pub async fn get_latest_blockhash(&self) -> Result<String, AppError> {
        let r = self
            .call("getLatestBlockhash", json!([{ "commitment": "confirmed" }]))
            .await?;
        r.get("value")
            .and_then(|v| v.get("blockhash"))
            .and_then(|h| h.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Real("getLatestBlockhash missing blockhash".into()))
    }

    /// Signature status: Some(true)=confirmed/finalized, Some(false)=processing,
    /// None=unknown.
    pub async fn get_signature_status(&self, sig: &str) -> Result<Option<bool>, AppError> {
        let r = self
            .call(
                "getSignatureStatuses",
                json!([[sig], { "searchTransactionHistory": true }]),
            )
            .await?;
        let arr = r.as_array().ok_or_else(|| AppError::Real("bad statuses array".into()))?;
        let first = arr.first();
        match first {
            None | Some(Value::Null) => Ok(None),
            Some(e) => {
                let confirmation = e
                    .get("confirmationStatus")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(Some(matches!(confirmation, "confirmed" | "finalized")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Start a tiny local HTTP server that returns canned JSON-RPC responses so
    /// the client logic is testable without any network access.
    #[tokio::test]
    async fn get_balance_parses() {
        let body = r#"{"jsonrpc":"2.0","result":{"value":1234567890},"id":1}"#;
        let (url, _svr) = serve(body).await;
        let rpc = SolanaRpc::new(&url);
        assert_eq!(rpc.get_balance("So11111111111111111111111111111111111111112").await.unwrap(), 1234567890);
    }

    #[tokio::test]
    async fn get_latest_blockhash_parses() {
        let body = r#"{"jsonrpc":"2.0","result":{"value":{"blockhash":"5Z2vTjZv8yYpYfQhgYkZsNLS3mQjPJBs1Z1Z1Z1Z1Z1Z1"}},"id":1}"#;
        let (url, _svr) = serve(body).await;
        let rpc = SolanaRpc::new(&url);
        assert!(rpc.get_latest_blockhash().await.unwrap().starts_with("5Z2v"));
    }

    #[tokio::test]
    async fn signature_status_parses_confirmed() {
        // Real shape: result = [status|null] (one entry per queried signature).
        let body = r#"{"jsonrpc":"2.0","result":[{"confirmationStatus":"confirmed"}],"id":1}"#;
        let (url, _svr) = serve(body).await;
        let rpc = SolanaRpc::new(&url);
        assert_eq!(rpc.get_signature_status("abc").await.unwrap(), Some(true));
    }

    #[tokio::test]
    async fn rpc_error_surfaces() {
        let body = r#"{"jsonrpc":"2.0","error":{"code":-32005,"message":"nope"},"id":1}"#;
        let (url, _svr) = serve(body).await;
        let rpc = SolanaRpc::new(&url);
        let err = rpc.get_balance("x").await.unwrap_err();
        assert!(err.to_string().contains("nope"));
    }

    /// Spin up a local listener returning `body` once, then returning 404s.
    async fn serve(body: &'static str) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}");
        let h = tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });
        (url, h)
    }
}
