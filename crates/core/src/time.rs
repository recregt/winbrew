//! Time helpers used by persistence and reporting code.
//!
//! The model and storage layers use these helpers to keep timestamps in one
//! canonical format and to avoid scattering raw chrono calls across call sites.

use chrono::Utc;

/// Return the current UTC timestamp in RFC 3339 second precision.
pub fn now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Return the current UTC timestamp as milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
