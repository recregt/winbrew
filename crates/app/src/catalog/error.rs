//! Shared catalog error types used by search and installer selection helpers.

use anyhow::Error;
use thiserror::Error;

/// Errors returned by catalog search helpers that need to distinguish a missing catalog from other failures.
#[derive(Debug)]
pub enum SearchError {
    /// The catalog database is not present on disk.
    CatalogUnavailable,
    /// Any other error surfaced while searching or resolving packages.
    Unexpected(Error),
}

/// Result type used by the high-level catalog search helpers.
pub type SearchResult<T> = std::result::Result<T, SearchError>;

impl From<Error> for SearchError {
    fn from(value: Error) -> Self {
        Self::Unexpected(value)
    }
}

/// Errors returned while selecting an installer from a package's installer list.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum InstallerSelectionError {
    #[error("catalog package has no installers")]
    NoInstallers,
}
