use std::error::Error as StdError;
use std::io;

use reqwest::Error as ReqwestError;
use thiserror::Error;

pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("failed to build HTTP client")]
    BuildClient {
        #[source]
        source: ReqwestError,
    },

    #[error("failed to request {label} {url}")]
    Request {
        label: String,
        url: String,
        #[source]
        source: ReqwestError,
    },

    #[error("{label} request failed")]
    RequestFailed {
        label: String,
        #[source]
        source: ReqwestError,
    },

    #[error("failed to create {label} download file at {path}")]
    CreateTempFile {
        label: String,
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to pre-allocate {label} download file")]
    Preallocate {
        label: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to read {label}")]
    Read {
        label: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to write {label} to disk")]
    Write {
        label: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to finalize {label} download buffer")]
    FinalizeBuffer {
        label: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to sync {label} download file")]
    Sync {
        label: String,
        #[source]
        source: io::Error,
    },

    #[error("{label} size mismatch: expected {expected}, got {actual}")]
    SizeMismatch {
        label: String,
        expected: u64,
        actual: u64,
    },

    #[error("download callback failed")]
    ChunkCallback {
        #[from]
        #[source]
        source: BoxError,
    },
}

pub type Result<T> = std::result::Result<T, DownloadError>;

impl DownloadError {
    pub(crate) fn build_client(source: ReqwestError) -> Self {
        Self::BuildClient { source }
    }

    pub(crate) fn request(
        label: impl Into<String>,
        url: impl Into<String>,
        source: ReqwestError,
    ) -> Self {
        Self::Request {
            label: label.into(),
            url: url.into(),
            source,
        }
    }

    pub(crate) fn request_failed(label: impl Into<String>, source: ReqwestError) -> Self {
        Self::RequestFailed {
            label: label.into(),
            source,
        }
    }

    pub(crate) fn create_temp_file(
        label: impl Into<String>,
        path: std::path::PathBuf,
        source: io::Error,
    ) -> Self {
        Self::CreateTempFile {
            label: label.into(),
            path,
            source,
        }
    }

    pub(crate) fn preallocate(label: impl Into<String>, source: io::Error) -> Self {
        Self::Preallocate {
            label: label.into(),
            source,
        }
    }

    pub(crate) fn read(label: impl Into<String>, source: io::Error) -> Self {
        Self::Read {
            label: label.into(),
            source,
        }
    }

    pub(crate) fn write(label: impl Into<String>, source: io::Error) -> Self {
        Self::Write {
            label: label.into(),
            source,
        }
    }

    pub(crate) fn finalize_buffer(label: impl Into<String>, source: io::Error) -> Self {
        Self::FinalizeBuffer {
            label: label.into(),
            source,
        }
    }

    pub(crate) fn sync(label: impl Into<String>, source: io::Error) -> Self {
        Self::Sync {
            label: label.into(),
            source,
        }
    }

    pub(crate) fn size_mismatch(label: impl Into<String>, expected: u64, actual: u64) -> Self {
        Self::SizeMismatch {
            label: label.into(),
            expected,
            actual,
        }
    }
}
