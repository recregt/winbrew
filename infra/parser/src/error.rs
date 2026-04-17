use std::io;
use std::path::PathBuf;

use rusqlite::Error as SqliteError;
use serde::Serialize;
use thiserror::Error;

use winbrew_models::shared::error::ModelError;

/// Error categories used to group parser failures for telemetry and logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ErrorCategory {
    Data,
    Validation,
    Infrastructure,
    Domain,
}

/// Parser-specific error values.
#[derive(Debug, Error, Serialize)]
pub enum ParserError {
    #[error("failed to decode fetched package payload")]
    Decode(
        #[from]
        #[serde(skip_serializing)]
        serde_json::Error,
    ),

    #[error("invalid scoop stream contract: {0}")]
    Contract(String),

    #[error("failed to decode fetched package payload on line {line}")]
    LineDecode {
        line: usize,
        #[source]
        #[serde(skip_serializing)]
        source: serde_json::Error,
    },

    #[error("failed to read or write parser artifact")]
    Io(
        #[from]
        #[serde(skip_serializing)]
        io::Error,
    ),

    #[error("failed to read or write parser artifact: {context}")]
    IoContext {
        #[source]
        #[serde(skip_serializing)]
        source: io::Error,
        context: String,
    },

    #[error("failed to access catalog database at {path}")]
    CatalogDb {
        path: PathBuf,
        #[source]
        #[serde(skip_serializing)]
        source: SqliteError,
    },

    #[error(transparent)]
    Model(
        #[from]
        #[serde(skip_serializing)]
        ModelError,
    ),
}

impl ParserError {
    /// Returns a parser error with extra context appended to contract errors.
    pub fn context(self, msg: impl Into<String>) -> Self {
        let msg = msg.into();

        match self {
            Self::Contract(base) => Self::Contract(format!("{base}: {msg}")),
            Self::Io(source) => Self::IoContext {
                source,
                context: msg,
            },
            Self::IoContext { source, context } => Self::IoContext {
                source,
                context: format!("{context}: {msg}"),
            },
            other => other,
        }
    }

    /// Creates an IO error with explicit context.
    pub fn io_with_context(source: io::Error, context: impl Into<String>) -> Self {
        Self::IoContext {
            source,
            context: context.into(),
        }
    }

    /// Creates a catalog database error with the path captured once.
    pub fn catalog_db(path: impl Into<PathBuf>, source: SqliteError) -> Self {
        Self::CatalogDb {
            path: path.into(),
            source,
        }
    }

    /// Returns a user-facing summary string.
    pub fn user_message(&self) -> String {
        match self {
            Self::Decode(_) | Self::LineDecode { .. } => {
                "Failed to decode package data".to_string()
            }
            Self::Contract(_) => "Invalid package data format".to_string(),
            Self::Io(_) | Self::IoContext { .. } => {
                "Parser artifact could not be read or written".to_string()
            }
            Self::CatalogDb { .. } => "Catalog database could not be accessed".to_string(),
            Self::Model(_) => "Model validation failed".to_string(),
        }
    }

    /// Returns whether the caller can retry the current operation.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Io(_) | Self::IoContext { .. } | Self::CatalogDb { .. }
        )
    }

    /// Returns a coarse category for telemetry or metrics.
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::Decode(_) | Self::LineDecode { .. } => ErrorCategory::Data,
            Self::Contract(_) => ErrorCategory::Validation,
            Self::Io(_) | Self::IoContext { .. } | Self::CatalogDb { .. } => {
                ErrorCategory::Infrastructure
            }
            Self::Model(_) => ErrorCategory::Domain,
        }
    }
}

impl From<(PathBuf, SqliteError)> for ParserError {
    fn from((path, source): (PathBuf, SqliteError)) -> Self {
        Self::catalog_db(path, source)
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorCategory, ParserError};
    use rusqlite::Error as SqliteError;

    #[test]
    fn categorizes_errors() {
        assert_eq!(
            ParserError::Contract("bad".to_string()).category(),
            ErrorCategory::Validation
        );
        assert_eq!(
            ParserError::Decode(serde_json::from_str::<serde_json::Value>("{").unwrap_err())
                .category(),
            ErrorCategory::Data
        );
        assert!(
            ParserError::io_with_context(std::io::Error::other("boom"), "open").is_recoverable()
        );
    }

    #[test]
    fn appends_context_to_contract_errors() {
        let error = ParserError::Contract("invalid shape".to_string()).context("winget envelope");

        match error {
            ParserError::Contract(message) => {
                assert_eq!(message, "invalid shape: winget envelope");
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn serializes_contextual_io_errors() {
        let error =
            ParserError::io_with_context(std::io::Error::other("boom"), "opening catalog.db");
        let json = serde_json::to_value(&error).expect("error should serialize");

        let text = json.to_string();
        assert!(text.contains("opening catalog.db"));
        assert!(!text.contains("boom"));
    }

    #[test]
    fn converts_path_and_sqlite_error() {
        let error: ParserError = (
            std::path::PathBuf::from("catalog.db"),
            SqliteError::InvalidQuery,
        )
            .into();

        match error {
            ParserError::CatalogDb { path, .. } => {
                assert_eq!(path, std::path::PathBuf::from("catalog.db"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
