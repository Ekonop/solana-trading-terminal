//! Time helpers. Timestamps are unix epoch MICROSECONDS as i64 (UTC), stored
//! directly as BIGINT — exact, timezone-free, index-friendly.

pub const MICROS_PER_SEC: i64 = 1_000_000;

/// Current unix time in microseconds (UTC).
pub fn now_micros() -> i64 {
    chrono::Utc::now().timestamp_micros()
}

/// RFC3339 UTC string for logs/UI.
pub fn fmt_ts(ts_micros: i64) -> String {
    let secs = ts_micros.div_euclid(MICROS_PER_SEC);
    let micros = ts_micros.rem_euclid(MICROS_PER_SEC);
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, (micros * 1_000) as u32);
    dt.map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| "invalid-ts".into())
}

/// Format a microsecond timestamp as a short local-readable string.
pub fn fmt_ts_short(ts_micros: i64) -> String {
    let secs = ts_micros.div_euclid(MICROS_PER_SEC);
    let micros = ts_micros.rem_euclid(MICROS_PER_SEC);
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, (micros * 1_000) as u32);
    dt.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".into())
}

/// Age of a timestamp in milliseconds.
pub fn age_ms(ts_micros: i64) -> i64 {
    now_micros().saturating_sub(ts_micros).div_euclid(1_000)
}