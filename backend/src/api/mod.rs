//! HTTP/WebSocket API.

pub mod error;
pub mod exports;
pub mod handlers;
pub mod real_handlers;
pub mod ws;

use std::str::FromStr;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::app::AppState;

pub const MAX_BODY_BYTES: usize = 256 * 1024;

/// Build the full API router.
pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/v1/status", get(handlers::status))
        .route("/api/v1/dashboard", get(handlers::dashboard))
        .route("/api/v1/portfolio", get(handlers::portfolio))
        .route("/api/v1/positions", get(handlers::positions))
        .route("/api/v1/orders", get(handlers::orders).post(handlers::submit_order))
        .route("/api/v1/orders/{id}/cancel", post(handlers::cancel_order))
        .route("/api/v1/fills", get(handlers::fills))
        .route("/api/v1/ledger", get(handlers::ledger))
        .route("/api/v1/strategies", get(handlers::strategies_list))
        .route("/api/v1/signals", get(handlers::signals_list))
        .route("/api/v1/strategies/{id}", get(handlers::strategy_detail))
        .route("/api/v1/strategies/{id}/config", put(handlers::strategy_update_config))
        .route("/api/v1/strategies/{id}/start", post(handlers::strategy_start))
        .route("/api/v1/strategies/{id}/pause", post(handlers::strategy_pause))
        .route("/api/v1/strategies/{id}/stop", post(handlers::strategy_stop))
        .route("/api/v1/markets", get(handlers::markets))
        .route("/api/v1/markets/{mint}", get(handlers::market_detail))
        .route("/api/v1/risk/events", get(handlers::risk_events))
        .route("/api/v1/system/events", get(handlers::system_events))
        .route("/api/v1/backtests", get(handlers::backtests_list).post(handlers::backtests_create))
        .route("/api/v1/backtests/{id}", get(handlers::backtest_detail))
        .route("/api/v1/settings", get(handlers::settings))
        .route("/api/v1/export/fills", get(exports::export_fills))
        .route("/api/v1/export/orders", get(exports::export_orders))
        .route("/api/v1/export/ledger", get(exports::export_ledger))
        .route("/api/v1/export/signals", get(exports::export_signals))
        .route("/api/v1/export/positions", get(exports::export_positions))
        .route("/api/v1/export/equity", get(exports::export_equity))
        .route("/api/v1/export/backtests", get(exports::export_backtests))
        .route("/api/v1/export/risk-events", get(exports::export_risk_events))
        .route("/api/v1/export/system-events", get(exports::export_system_events))
        .route("/api/v1/agent/overview", get(handlers::agent_overview))
        .route("/api/v1/stop-all", post(handlers::stop_all))
        .route("/api/v1/real/status", get(real_handlers::real_status))
        .route("/api/v1/real/arm", post(real_handlers::real_arm))
        .route("/api/v1/real/disarm", post(real_handlers::real_disarm))
        .route("/api/v1/real/wallet", get(real_handlers::real_wallet))
        .route("/api/v1/real/quote-dry", get(real_handlers::real_quote_dry))
        .route("/api/v1/real/sol-price", get(real_handlers::real_sol_price))
        .route("/api/v1/real/execute-buy", post(real_handlers::real_execute_buy))
        .route("/api/v1/demo/reset", post(handlers::reset_demo_data))
        .route("/api/v1/ws", get(ws::ws_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub(crate) fn query_limit(limit: Option<String>, offset: Option<String>) -> crate::repos::Page {
    let l = limit.and_then(|s| u32::from_str(&s).ok());
    let o = offset.and_then(|s| u32::from_str(&s).ok());
    crate::repos::Page::new(l, o)
}