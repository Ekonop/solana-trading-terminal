//! Axum error conversion for typed API errors.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::errors::{ApiErrorBody, AppError};

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = ApiErrorBody::from_app(&self);
        (status, Json(body)).into_response()
    }
}

/// Ergonomic wrapper: `Result<T, AppError>` in handlers.
pub type ApiResult<T> = Result<T, AppError>;