use anyhow::Error;
use std::io;
use thiserror::Error;

use super::state::InstallStateError;
use crate::core::cancel::CancellationError;
use crate::core::hash::{HashAlgorithm, HashError};
use crate::services::catalog::InstallerSelectionError;

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub name: String,
    pub version: String,
    pub install_dir: String,
}

#[derive(Debug, Clone)]
pub struct InstallOutcome {
    pub result: InstallResult,
    pub legacy_checksum_algorithms: Vec<HashAlgorithm>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRollbackKind {
    Failed,
    Cancelled,
}

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("package '{name}' is already installed")]
    AlreadyInstalled { name: String },

    #[error("package '{name}' is already being installed")]
    AlreadyInstalling { name: String },

    #[error("package '{name}' is currently updating")]
    CurrentlyUpdating { name: String },

    #[error("installer checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("{algorithm} checksums are disabled by default for security")]
    LegacyChecksumAlgorithm { algorithm: HashAlgorithm },

    #[error("cancelled")]
    Cancelled,

    #[error(transparent)]
    Unexpected(Error),
}

pub type Result<T> = std::result::Result<T, InstallError>;

impl InstallError {
    pub fn rollback_kind(&self) -> InstallRollbackKind {
        if matches!(self, Self::Cancelled) {
            InstallRollbackKind::Cancelled
        } else {
            InstallRollbackKind::Failed
        }
    }
}

impl From<InstallStateError> for InstallError {
    fn from(value: InstallStateError) -> Self {
        match value {
            InstallStateError::AlreadyInstalled { name } => Self::AlreadyInstalled { name },
            InstallStateError::AlreadyInstalling { name } => Self::AlreadyInstalling { name },
            InstallStateError::CurrentlyUpdating { name } => Self::CurrentlyUpdating { name },
            other => Self::Unexpected(Error::new(other)),
        }
    }
}

impl From<HashError> for InstallError {
    fn from(value: HashError) -> Self {
        match value {
            HashError::ChecksumMismatch { expected, actual } => {
                Self::ChecksumMismatch { expected, actual }
            }
            HashError::LegacyChecksumAlgorithm { algorithm } => {
                Self::LegacyChecksumAlgorithm { algorithm }
            }
        }
    }
}

impl From<CancellationError> for InstallError {
    fn from(_: CancellationError) -> Self {
        Self::Cancelled
    }
}

impl From<InstallerSelectionError> for InstallError {
    fn from(value: InstallerSelectionError) -> Self {
        Self::Unexpected(Error::new(value))
    }
}

impl From<io::Error> for InstallError {
    fn from(value: io::Error) -> Self {
        Self::Unexpected(Error::new(value))
    }
}

impl From<Error> for InstallError {
    fn from(value: Error) -> Self {
        if let Some(hash_error) = value.downcast_ref::<HashError>() {
            return Self::from(hash_error.clone());
        }

        if let Some(selection_error) = value.downcast_ref::<InstallerSelectionError>() {
            return Self::from(*selection_error);
        }

        if value.downcast_ref::<CancellationError>().is_some() {
            return Self::Cancelled;
        }

        Self::Unexpected(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallError, InstallRollbackKind, InstallStateError};
    use crate::core::cancel::CancellationError;
    use crate::core::hash::{HashAlgorithm, HashError};

    #[test]
    fn maps_state_conflicts_to_user_facing_errors() {
        let err = InstallError::from(InstallStateError::AlreadyInstalled {
            name: "Contoso.App".to_string(),
        });

        assert!(matches!(err, InstallError::AlreadyInstalled { .. }));
    }

    #[test]
    fn maps_hash_errors_to_user_facing_errors() {
        let err = InstallError::from(HashError::LegacyChecksumAlgorithm {
            algorithm: HashAlgorithm::Sha1,
        });

        assert!(matches!(err, InstallError::LegacyChecksumAlgorithm { .. }));
    }

    #[test]
    fn maps_cancellation_to_cancelled() {
        let err = InstallError::from(CancellationError);

        assert!(matches!(err, InstallError::Cancelled));
    }

    #[test]
    fn rollback_kind_is_cancelled_only_for_cancelled_errors() {
        assert_eq!(
            InstallError::Cancelled.rollback_kind(),
            InstallRollbackKind::Cancelled
        );
        assert_eq!(
            InstallError::from(InstallStateError::AlreadyInstalled {
                name: "Contoso.App".to_string(),
            })
            .rollback_kind(),
            InstallRollbackKind::Failed
        );
    }
}
