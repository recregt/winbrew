//! Canonical error type for model parsing, validation, and contract checks.
//!
//! Use `ModelError` whenever a model cannot be parsed, validated, or mapped to
//! a stable contract. The variants are intentionally narrow so higher layers can
//! format user-facing diagnostics without guessing at the original failure.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A required field was empty after trimming whitespace.
    #[error("{field} cannot be empty")]
    EmptyField { field: &'static str },
    /// A URL failed parsing or used a non-HTTP scheme.
    #[error("invalid url for {field}: {value}")]
    InvalidUrl { field: &'static str, value: String },
    /// A checksum field was blank or contained non-hexadecimal data.
    #[error("invalid hash for {field}: {value}")]
    InvalidHash { field: &'static str, value: String },
    /// A semantic version string could not be parsed or normalized.
    #[error("invalid version {value}: {reason}")]
    InvalidVersion { value: String, reason: String },
    /// A package id could not be parsed from `@winget` or `@scoop` syntax.
    #[error("invalid package id {value}: {reason}")]
    InvalidPackageId { value: String, reason: String },
    /// An enum value was outside the accepted vocabulary for the field.
    #[error("invalid {field}: {value}")]
    InvalidEnumValue { field: &'static str, value: String },
    /// A typed value did not match the source that the schema expected.
    #[error("source mismatch for {field}: expected {expected}, got {actual}")]
    SourceMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    /// A higher-level invariant or contract was violated.
    #[error("invalid contract for {field}: {reason}")]
    InvalidContract { field: &'static str, reason: String },
}

impl ModelError {
    /// Build an empty-field error for the given logical field name.
    pub fn empty(field: &'static str) -> Self {
        Self::EmptyField { field }
    }

    /// Build a URL validation error for the given logical field name.
    pub fn invalid_url(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidUrl {
            field,
            value: value.into(),
        }
    }

    /// Build a hash validation error for the given logical field name.
    pub fn invalid_hash(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidHash {
            field,
            value: value.into(),
        }
    }

    /// Build a version parsing error for the original value and reason.
    pub fn invalid_version(value: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidVersion {
            value: value.into(),
            reason: reason.into(),
        }
    }

    /// Build a package-id parsing error for the original value and reason.
    pub fn invalid_package_id(value: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidPackageId {
            value: value.into(),
            reason: reason.into(),
        }
    }

    /// Build a generic enum-vocabulary error for the given field.
    pub fn invalid_enum_value(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidEnumValue {
            field,
            value: value.into(),
        }
    }

    /// Build a source-mismatch error when a typed record disagrees with the schema.
    pub fn source_mismatch(
        field: &'static str,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::SourceMismatch {
            field,
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Build a contract error for a schema or invariant violation.
    pub fn invalid_contract(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidContract {
            field,
            reason: reason.into(),
        }
    }
}
