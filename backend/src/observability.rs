//! Structured logging for the whole application.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialise tracing with a single stderr subscriber (pretty console layer
/// with env filtering).
pub fn init() {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(
            std::env::var("STT_LOG")
                .unwrap_or_else(|_| "info".into())
                .parse()
                .unwrap_or(tracing::Level::INFO.into()),
        )
        // pglite internals are very chatty at debug; keep them quiet.
        .add_directive("pglite=info".parse().unwrap_or_else(|_| tracing::Level::WARN.into()))
        .add_directive("hyper=info".parse().unwrap_or_else(|_| tracing::Level::INFO.into()));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_ansi(true),
        )
        .init();
}