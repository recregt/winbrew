//! Error normalization and installer-selection helpers for installation.
//!
//! This module keeps the install boundary stable by translating lower-level
//! failures into a smaller set of user-facing errors. It also wraps catalog
//! installer selection so the outer workflow does not need to know the catalog
//! policy for choosing between multiple installer records.

use anyhow::Error;
use std::io;
use thiserror::Error;

use super::state::InstallStateError;
use crate::catalog::{self, InstallerSelectionError, SelectionContext};
use crate::core::cancel::CancellationError;
use crate::core::hash::HashError;
use crate::models::catalog::CatalogInstaller;
use crate::models::domains::install::InstallFailureClass;
use crate::models::domains::shared::HashAlgorithm;
use crate::windows::HostProfile;

/// Select the installer that the catalog policy considers best for the package.
///
/// The underlying catalog layer owns the actual ranking logic. This helper only
/// forwards the current host profile and preserves the selector's explicit
/// failure reasons for the install workflow.
pub(crate) fn select_installer(
    installers: &[CatalogInstaller],
    selection_context: SelectionContext,
) -> std::result::Result<CatalogInstaller, InstallerSelectionError> {
    catalog::select_installer(installers, selection_context)
}

/// User-facing error type produced by the install pipeline.
///
/// The variants intentionally map to coarse categories rather than exposing the
/// raw implementation details from catalog resolution, checksum verification,
/// cancellation, and rollback. This keeps the CLI behavior predictable while
/// still preserving enough context for diagnostics and testing.
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

    #[error("catalog package has no installers")]
    NoInstallers,

    #[error("no installer matches this host ({host})")]
    NoCompatibleInstaller { host: HostProfile },

    #[error("no installer matches this host's install scope ({host})")]
    NoScopeCompatibleInstaller { host: HostProfile },

    #[error("runtime bootstrap for {runtime} was declined")]
    RuntimeBootstrapDeclined { runtime: String },

    #[error("cancelled")]
    Cancelled,

    #[error(transparent)]
    Unexpected(Error),
}

/// Convenience result type for install operations.
pub type Result<T> = std::result::Result<T, InstallError>;

impl InstallError {
    /// Group the error into a coarse failure class for reporting and rollback.
    pub fn failure_class(&self) -> InstallFailureClass {
        match self {
            Self::AlreadyInstalled { .. }
            | Self::AlreadyInstalling { .. }
            | Self::CurrentlyUpdating { .. } => InstallFailureClass::Preflight,
            Self::ChecksumMismatch { .. } | Self::LegacyChecksumAlgorithm { .. } => {
                InstallFailureClass::Verification
            }
            Self::NoInstallers
            | Self::NoCompatibleInstaller { .. }
            | Self::NoScopeCompatibleInstaller { .. }
            | Self::RuntimeBootstrapDeclined { .. } => InstallFailureClass::Preflight,
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
        match value {
            InstallerSelectionError::NoInstallers => Self::NoInstallers,
            InstallerSelectionError::PlatformMismatch { host } => {
                Self::NoCompatibleInstaller { host }
            }
            InstallerSelectionError::ScopeMismatch { host } => {
                Self::NoScopeCompatibleInstaller { host }
            }
        }
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
    use crate::catalog::InstallerSelectionError;
    use crate::core::cancel::CancellationError;
    use crate::core::hash::HashError;
    use crate::models::domains::install::Architecture;
    use crate::models::domains::install::InstallFailureClass;
    use crate::models::domains::shared::HashAlgorithm;
    use crate::windows::HostProfile;

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
    fn maps_selection_failures_to_user_facing_errors() {
        let err = InstallError::from(InstallerSelectionError::NoInstallers);
        assert!(matches!(err, InstallError::NoInstallers));

        let err = InstallError::from(InstallerSelectionError::PlatformMismatch {
            host: HostProfile {
                is_server: true,
                architecture: Architecture::Arm64,
            },
        });
        assert!(matches!(err, InstallError::NoCompatibleInstaller { .. }));

        let err = InstallError::from(InstallerSelectionError::ScopeMismatch {
            host: HostProfile {
                is_server: false,
                architecture: Architecture::X64,
            },
        });
        assert!(matches!(
            err,
            InstallError::NoScopeCompatibleInstaller { .. }
        ));
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
        assert_eq!(
            InstallError::RuntimeBootstrapDeclined {
                runtime: "7-Zip runtime".to_string(),
            }
            .failure_class(),
            InstallFailureClass::Preflight
        );
    }
}
