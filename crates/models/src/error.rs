use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModelError {
    #[error("{field} cannot be empty")]
    EmptyField { field: &'static str },
    #[error("invalid url for {field}: {value}")]
    InvalidUrl { field: &'static str, value: String },
    #[error("invalid hash for {field}: {value}")]
    InvalidHash { field: &'static str, value: String },
    #[error("invalid version {value}: {reason}")]
    InvalidVersion { value: String, reason: String },
    #[error("invalid package id {value}: {reason}")]
    InvalidPackageId { value: String, reason: String },
    #[error("invalid {field}: {value}")]
    InvalidEnumValue { field: &'static str, value: String },
}

impl ModelError {
    pub fn empty(field: &'static str) -> Self {
        Self::EmptyField { field }
    }

    pub fn invalid_url(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidUrl {
            field,
            value: value.into(),
        }
    }

    pub fn invalid_hash(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidHash {
            field,
            value: value.into(),
        }
    }

    pub fn invalid_version(value: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidVersion {
            value: value.into(),
            reason: reason.into(),
        }
    }

    pub fn invalid_package_id(value: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidPackageId {
            value: value.into(),
            reason: reason.into(),
        }
    }

    pub fn invalid_enum_value(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidEnumValue {
            field,
            value: value.into(),
        }
    }
}
