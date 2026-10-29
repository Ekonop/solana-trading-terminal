use solana_trading_terminal::{config::Config, observability, AppError};

#[tokio::main]
async fn main() {
    observability::init();
    // Load server-side secrets from a gitignored `.env` (if present) BEFORE
    // config resolution, so STT_REAL_* overrides come from there. Secrets are
    // resolved exclusively server-side and never exposed to the client.
    let _ = solana_trading_terminal::secrets::load_dotenv_if_present().map_err(|e| {
        tracing::warn!(error = %e, "optional .env load failed; continuing with process env");
    });
    let _ = dotenvy::dotenv().ok();

    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e}");
            std::process::exit(2);
        }
    };

    if let Err(e) = solana_trading_terminal::app::run(cfg).await {
        tracing::error!("fatal: {e}");
        std::process::exit(1);
    }
}

// Keep the type reachable so dead-code analysis is honest about plumbing.
#[allow(dead_code)]
fn _typecheck(_: &AppError) {}