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

/// Reject values that can never be legitimate identity/display text: NUL
/// bytes, or a value that is *exactly* `.` or `..` after trimming.
///
/// This is intentionally narrow. Fields like a catalog package's display
/// `name` routinely contain characters such as `/` or `:` as ordinary text
/// (for example `"AMD Software: Cloud Edition"` or
/// `"Visual Studio / Code for Command Palette"` are real Winget package
/// names), so this helper does not reject those -- it only catches values
/// that could never be legitimate text at all. Callers that turn a value
/// into a single filesystem path component (a directory or file name joined
/// onto a managed root) are responsible for sanitizing separators and
/// drive/stream markers themselves (see
/// `winbrew_core::paths::ResolvedPaths::package_install_dir`), since that is
/// a presentation/storage concern the shared model layer should not decide
/// for every caller.
pub fn ensure_safe_path_component(field: &'static str, value: &str) -> Result<(), ModelError> {
    let trimmed = value.trim();

    let is_unsafe = trimmed.is_empty() || trimmed == "." || trimmed == ".." || value.contains('\0');

    if is_unsafe {
        Err(ModelError::invalid_contract(
            field,
            format!("{value:?} is not safe to use as a path component"),
        ))
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
