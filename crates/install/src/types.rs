use anyhow::Error;
use std::io;
use thiserror::Error;

use super::state::InstallStateError;
use crate::cancel::CancellationError;
use crate::catalog::InstallerSelectionError;
use crate::core::hash::HashError;
use crate::models::HashAlgorithm;
use winbrew_models::InstallFailureClass;

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
    pub fn failure_class(&self) -> InstallFailureClass {
        match self {
            Self::AlreadyInstalled { .. }
            | Self::AlreadyInstalling { .. }
            | Self::CurrentlyUpdating { .. } => InstallFailureClass::Preflight,
            Self::ChecksumMismatch { .. } | Self::LegacyChecksumAlgorithm { .. } => {
                InstallFailureClass::Verification
            }
            Self::Cancelled => InstallFailureClass::Cancelled,
            Self::Unexpected(_) => InstallFailureClass::Runtime,
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
    use super::{InstallError, InstallStateError};
    use crate::cancel::CancellationError;
    use crate::core::hash::HashError;
    use crate::models::HashAlgorithm;
    use winbrew_models::InstallFailureClass;

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
    fn failure_class_groups_expected_variants() {
        assert_eq!(
            InstallError::from(InstallStateError::AlreadyInstalling {
                name: "Contoso.App".to_string(),
            })
            .failure_class(),
            InstallFailureClass::Preflight
        );
        assert_eq!(
            InstallError::from(HashError::ChecksumMismatch {
                expected: "a".to_string(),
                actual: "b".to_string(),
            })
            .failure_class(),
            InstallFailureClass::Verification
        );
        assert_eq!(
            InstallError::Cancelled.failure_class(),
            InstallFailureClass::Cancelled
        );
    }
}
