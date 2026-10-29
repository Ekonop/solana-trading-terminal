use solana_trading_terminal::{config::Config, observability, AppError};

#[tokio::main]
async fn main() {
    observability::init();
    dotenvy::dotenv().ok();

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