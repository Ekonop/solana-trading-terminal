//! Typed conversion helpers between pglite rows and domain values.
//!
//! All financial numbers travel as `rust_decimal::Decimal` (NUMERIC columns)
//! or integers (lamports / raw base units) — never as binary floats. These
//! helpers centralise the one place where pglite row types are touched so the
//! rest of the application stays database-agnostic.

use pglite::Row;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::errors::AppError;

pub fn row_decimal(row: &Row, name: &str) -> Result<Decimal, AppError> {
    row.try_get::<Decimal>(name)
        .map_err(|e| AppError::Database(format!("bad numeric column '{name}': {e}")))
}

pub fn row_decimal_opt(row: &Row, name: &str) -> Result<Option<Decimal>, AppError> {
    row.try_get::<Option<Decimal>>(name)
        .map_err(|e| AppError::Database(format!("bad numeric column '{name}': {e}")))
}

pub fn row_i64(row: &Row, name: &str) -> Result<i64, AppError> {
    row.try_get::<i64>(name)
        .map_err(|e| AppError::Database(format!("bad integer column '{name}': {e}")))
}

pub fn row_i64_opt(row: &Row, name: &str) -> Result<Option<i64>, AppError> {
    row.try_get::<Option<i64>>(name)
        .map_err(|e| AppError::Database(format!("bad integer column '{name}': {e}")))
}

pub fn row_i32(row: &Row, name: &str) -> Result<i32, AppError> {
    row.try_get::<i32>(name)
        .map_err(|e| AppError::Database(format!("bad integer column '{name}': {e}")))
}

pub fn row_bool(row: &Row, name: &str) -> Result<bool, AppError> {
    row.try_get::<bool>(name)
        .map_err(|e| AppError::Database(format!("bad boolean column '{name}': {e}")))
}

pub fn row_str(row: &Row, name: &str) -> Result<String, AppError> {
    row.try_get::<&str>(name)
        .map(str::to_string)
        .map_err(|e| AppError::Database(format!("bad text column '{name}': {e}")))
}

pub fn row_str_opt(row: &Row, name: &str) -> Result<Option<String>, AppError> {
    row.try_get::<Option<&str>>(name)
        .map(|o| o.map(str::to_string))
        .map_err(|e| AppError::Database(format!("bad text column '{name}': {e}")))
}

/// Parse a validated decimal from a string (used for config values and API
/// inputs). Rejects NaN/Inf and anything non-numeric.
pub fn parse_decimal(s: &str, field: &str) -> Result<Decimal, AppError> {
    let d = s.parse::<Decimal>().map_err(|_| {
        AppError::Validation {
            field: field.to_string(),
            message: format!("'{s}' is not a valid decimal number"),
        }
    })?;
    // rust_decimal's parser rejects NaN/Inf by itself; is_finite() is not
    // part of Decimal's API, so the parse check above is authoritative.
    Ok(d)
}

/// Basis points as a Decimal fraction (1 bp = 0.0001).
pub fn bps_to_fraction(bps: i64) -> Decimal {
    Decimal::from(bps) / Decimal::from(10_000)
}

/// Fraction (0..1) expressed in basis points.
pub fn fraction_to_bps(f: Decimal) -> i64 {
    (f * Decimal::from(10_000))
        .round_dp(0)
        .to_i64()
        .unwrap_or(0)
}

pub const LAMPORTS_PER_SOL: i64 = 1_000_000_000;

/// Human-readable lamports amount ("1.234" SOL) for display only.
pub fn lamports_to_sol_str(lamports: i64) -> String {
    let sol = Decimal::from(lamports) / Decimal::from(LAMPORTS_PER_SOL);
    format!("{sol:.6}")
}

pub fn sol_str_to_lamports(s: &str) -> Result<i64, AppError> {
    let d = parse_decimal(s, "amount")?;
    let lamports = (d * Decimal::from(LAMPORTS_PER_SOL))
        .round_dp(0)
        .to_i64()
        .ok_or_else(|| AppError::Validation {
            field: "amount".into(),
            message: "amount out of range".into(),
        })?;
    if lamports < 0 {
        return Err(AppError::Validation {
            field: "amount".into(),
            message: "amount must be non-negative".into(),
        });
    }
    Ok(lamports)
}