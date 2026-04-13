//! Shared validation helpers for model invariants.
//!
//! Validation in this crate is intentionally lightweight: types implement the
//! `Validate` trait when they can check their own invariants, and helper
//! functions provide reusable checks for common string-based contracts.

use super::error::ModelError;

/// A model type that can verify its own invariants.
pub trait Validate {
    fn validate(&self) -> Result<(), ModelError>;
}

/// Reject values that are empty after trimming whitespace.
pub fn ensure_non_empty(field: &'static str, value: &str) -> Result<(), ModelError> {
    if value.trim().is_empty() {
        Err(ModelError::empty(field))
    } else {
        Ok(())
    }
}

/// Accept only `http` and `https` URLs.
pub fn ensure_http_url(field: &'static str, value: &str) -> Result<(), ModelError> {
    let parsed = url::Url::parse(value)
        .map_err(|err| ModelError::invalid_url(field, format!("{value} ({err})")))?;

    match parsed.scheme() {
        "http" | "https" => Ok(()),
        other => Err(ModelError::invalid_url(
            field,
            format!("{value} (unsupported scheme {other})"),
        )),
    }
}

/// Accept hexadecimal hashes with or without a known algorithm prefix.
pub fn ensure_hash(field: &'static str, value: &str) -> Result<(), ModelError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ModelError::invalid_hash(field, value));
    }

    let candidate = normalized
        .strip_prefix("sha256:")
        .or_else(|| normalized.strip_prefix("sha1:"))
        .or_else(|| normalized.strip_prefix("md5:"))
        .or_else(|| normalized.strip_prefix("sha512:"))
        .unwrap_or(normalized);

    if candidate.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(ModelError::invalid_hash(field, value))
    }
}
