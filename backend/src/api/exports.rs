//! Research export endpoints — CSV dumps of the paper-trading data model.
//!
//! These exist so researchers can pull clean, column-stable CSVs into
//! external tooling (pandas, spreadsheets, notebooks). They are read-only.

use axum::extract::State;
use axum::response::Response;

use crate::api::error::ApiResult;
use crate::app::AppState;
use crate::repos;

const CONTENT_TYPE: &str = "text/csv; charset=utf-8";

/// Small helper: build a CSV response with a Content-Disposition attachment.
fn csv_response(filename: &str, csv: String, truncated: bool) -> Response {
    let disposition = format!("attachment; filename=\"{filename}\"");
    let mut res = Response::new(axum::body::Body::from(csv));
    let headers = res.headers_mut();
    headers.insert("Content-Type", CONTENT_TYPE.parse().unwrap());
    headers.insert(
        "Content-Disposition",
        disposition.parse().expect("valid header"),
    );
    if truncated {
        headers.insert(
            "X-Export-Truncated",
            "true".parse().expect("valid header"),
        );
    }
    res
}

pub async fn export_fills(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) = repos::exports::export_fills(
        &st.db,
        &st.account_id,
        repos::exports::EXPORT_LIMIT,
    )
    .await?;
    Ok(csv_response("fills.csv", csv, truncated))
}

pub async fn export_orders(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) = repos::exports::export_orders(
        &st.db,
        &st.account_id,
        repos::exports::EXPORT_LIMIT,
    )
    .await?;
    Ok(csv_response("orders.csv", csv, truncated))
}

pub async fn export_ledger(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) = repos::exports::export_ledger(
        &st.db,
        &st.account_id,
        repos::exports::EXPORT_LIMIT,
    )
    .await?;
    Ok(csv_response("ledger.csv", csv, truncated))
}

pub async fn export_signals(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) = repos::exports::export_signals(&st.db, repos::exports::EXPORT_LIMIT).await?;
    Ok(csv_response("signals.csv", csv, truncated))
}

pub async fn export_positions(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) = repos::exports::export_positions(&st.db, &st.account_id).await?;
    Ok(csv_response("positions.csv", csv, truncated))
}

pub async fn export_equity(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) = repos::exports::export_equity(
        &st.db,
        &st.account_id,
        repos::exports::EXPORT_LIMIT,
    )
    .await?;
    Ok(csv_response("equity.csv", csv, truncated))
}

pub async fn export_backtests(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) =
        repos::exports::export_backtests(&st.db, repos::exports::EXPORT_LIMIT).await?;
    Ok(csv_response("backtests.csv", csv, truncated))
}

pub async fn export_risk_events(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) =
        repos::exports::export_risk_events(&st.db, repos::exports::EXPORT_LIMIT).await?;
    Ok(csv_response("risk-events.csv", csv, truncated))
}

pub async fn export_system_events(State(st): State<AppState>) -> ApiResult<Response> {
    let (csv, truncated) =
        repos::exports::export_system_events(&st.db, repos::exports::EXPORT_LIMIT).await?;
    Ok(csv_response("system-events.csv", csv, truncated))
}