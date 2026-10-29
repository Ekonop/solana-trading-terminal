//! Typed application errors. Every subsystem reports errors through this type
//! so the API can translate them into consistent JSON payloads and the
//! observability layer can log them with subsystem context.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("database unavailable: {0}")]
    DatabaseUnavailable(String),

    #[error("migration error: {0}")]
    Migration(String),

    #[error("validation error: {field}: {message}")]
    Validation { field: String, message: String },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("risk manager rejected the order: {0}")]
    RiskRejected(String),

    #[error("insufficient paper balance")]
    InsufficientBalance,

    #[error("insufficient position quantity for mint {mint}")]
    InsufficientPosition { mint: String },

    #[error("market data unavailable: {0}")]
    MarketData(String),

    #[error("strategy error: {0}")]
    Strategy(String),

    #[error("invalid lifecycle transition: {0}")]
    Lifecycle(String),

    /// Real-mode errors: disarmed, wallet, chain, jupiter, reconciliation.
    #[error("real-mode: {0}")]
    Real(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Human-readable machine code for the API layer.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Config(_) => "config_error",
            AppError::Database(_) => "database_error",
            AppError::DatabaseUnavailable(_) => "database_unavailable",
            AppError::Migration(_) => "migration_error",
            AppError::Validation { .. } => "validation_error",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::RiskRejected(_) => "risk_rejected",
            AppError::InsufficientBalance => "insufficient_balance",
            AppError::InsufficientPosition { .. } => "insufficient_position",
            AppError::MarketData(_) => "market_data_unavailable",
            AppError::Strategy(_) => "strategy_error",
            AppError::Lifecycle(_) => "lifecycle_error",
            AppError::Real(_) => "real_mode_error",
            AppError::Internal(_) => "internal_error",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            AppError::Config(_) => 500,
            AppError::Database(_) => 500,
            AppError::DatabaseUnavailable(_) => 503,
            AppError::Migration(_) => 500,
            AppError::Validation { .. } => 400,
            AppError::NotFound(_) => 404,
            AppError::Conflict(_) => 409,
            AppError::RiskRejected(_) => 422,
            AppError::InsufficientBalance => 422,
            AppError::InsufficientPosition { .. } => 422,
            AppError::MarketData(_) => 503,
            AppError::Strategy(_) => 500,
            AppError::Lifecycle(_) => 409,
            AppError::Real(_) => 422,
            AppError::Internal(_) => 500,
        }
    }
}

impl From<pglite::Error> for AppError {
    fn from(e: pglite::Error) -> Self {
        use pglite::Error as Pe;
        match e {
            Pe::AlreadyOpen | Pe::ReopenUnsupported => {
                AppError::DatabaseUnavailable(e.to_string())
            }
            Pe::Closed => AppError::DatabaseUnavailable("database is closed".into()),
            other => AppError::Database(other.to_string()),
        }
    }
}

/// Consistent API error payload.
#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub error: String,
    pub code: String,
    pub message: String,
}

impl ApiErrorBody {
    pub fn from_app(e: &AppError) -> Self {
        ApiErrorBody {
            error: e.code().to_string(),
            code: e.code().to_string(),
            message: e.to_string(),
        }
    }
}