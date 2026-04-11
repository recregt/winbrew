//! Database and filesystem state helpers for installation.
//!
//! This module is responsible for the persistence side of the install flow.
//! It validates whether a package can be installed, removes stale failed state,
//! records the package as installing, and flips the status to either installed
//! or failed once the engine phase completes.
//!
//! Keeping these transitions isolated makes the outer install orchestration
//! easier to reason about and gives rollback a single place to update package
//! status.

use std::path::Path;
use thiserror::Error;

use crate::core::fs::cleanup_path;
use crate::core::now;
use crate::storage;
use winbrew_models::{
    EngineInstallReceipt, EngineKind, EngineMetadata, InstallerType, MsiInventoryReceipt, Package,
    PackageStatus,
};

/// Errors raised while preparing or updating install state.
#[derive(Debug, Error)]
pub enum InstallStateError {
    #[error("failed to read install state for '{name}'")]
    LookupFailed {
        name: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("package '{name}' is already installed")]
    AlreadyInstalled { name: String },

    #[error("package '{name}' is already being installed")]
    AlreadyInstalling { name: String },

    #[error("package '{name}' is currently updating")]
    CurrentlyUpdating { name: String },

    #[error("failed to delete failed install record for '{name}'")]
    DeleteFailed {
        name: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("failed to clean up install directory at {path}")]
    CleanupFailed {
        path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("failed to update install state during {operation}")]
    DatabaseOperationFailed {
        operation: &'static str,
        #[source]
        source: anyhow::Error,
    },
}

/// Convenience result type for install-state operations.
pub type Result<T> = std::result::Result<T, InstallStateError>;

/// Validate the target install path and clear stale failed state if present.
///
/// This function enforces the database-level install preconditions before any
/// download work begins. It rejects packages that are already installed,
/// already installing, or currently updating. If the previous attempt failed,
/// the stale package row is removed and the install directory is cleaned so the
/// next attempt starts from a known-good state.
pub fn prepare_install_target(
    conn: &crate::storage::DbConnection,
    name: &str,
    install_dir: &Path,
) -> Result<()> {
    if let Some(existing) =
        storage::get_package(conn, name).map_err(|source| InstallStateError::LookupFailed {
            name: name.to_string(),
            source,
        })?
    {
        match existing.status {
            PackageStatus::Ok => {
                return Err(InstallStateError::AlreadyInstalled {
                    name: name.to_string(),
                });
            }
            PackageStatus::Installing => {
                return Err(InstallStateError::AlreadyInstalling {
                    name: name.to_string(),
                });
            }
            PackageStatus::Updating => {
                return Err(InstallStateError::CurrentlyUpdating {
                    name: name.to_string(),
                });
            }
            PackageStatus::Failed => {
                storage::delete_package(conn, name).map_err(|source| {
                    InstallStateError::DeleteFailed {
                        name: name.to_string(),
                        source,
                    }
                })?;
                cleanup_path(install_dir).map_err(|source| InstallStateError::CleanupFailed {
                    path: install_dir.to_string_lossy().into_owned(),
                    source: source.into(),
                })?;
            }
        }
    } else if install_dir.exists() {
        cleanup_path(install_dir).map_err(|source| InstallStateError::CleanupFailed {
            path: install_dir.to_string_lossy().into_owned(),
            source: source.into(),
        })?;
    }

    Ok(())
}

/// Insert a package record marked as installing.
///
/// The record captures the package metadata and provisional install directory so
/// the database reflects that work is in progress before the payload download
/// starts.
pub fn mark_installing(
    conn: &crate::storage::DbConnection,
    name: impl Into<String>,
    version: impl Into<String>,
    kind: InstallerType,
    engine_kind: EngineKind,
    install_dir: &Path,
) -> Result<()> {
    let package = installing_package(name, version, kind, engine_kind, install_dir);
    storage::insert_package(conn, &package).map_err(|source| {
        InstallStateError::DatabaseOperationFailed {
            operation: "marking package as installing",
            source,
        }
    })
}

/// Mark a package as successfully installed.
///
/// The engine receipt carries any metadata needed for future removal or repair,
/// including MSIX-specific package identity data when the engine was able to
/// resolve it during install.
pub fn mark_ok(
    conn: &mut crate::storage::DbConnection,
    name: &str,
    engine_receipt: &EngineInstallReceipt,
) -> Result<()> {
    let installed_at = now();
    let tx = conn
        .transaction()
        .map_err(|source| InstallStateError::DatabaseOperationFailed {
            operation: "starting install commit transaction",
            source: source.into(),
        })?;

    storage::update_status_and_engine_metadata(
        &tx,
        name,
        PackageStatus::Ok,
        engine_receipt.engine_metadata.as_ref(),
        &installed_at,
    )
    .map_err(|source| InstallStateError::DatabaseOperationFailed {
        operation: "marking package as installed",
        source,
    })?;

    if let Some(receipt) = msi_inventory_receipt(name, engine_receipt.engine_metadata.as_ref()) {
        storage::upsert_receipt(&tx, &receipt).map_err(|source| {
            InstallStateError::DatabaseOperationFailed {
                operation: "recording MSI receipt",
                source,
            }
        })?;
    }

    tx.commit()
        .map_err(|source| InstallStateError::DatabaseOperationFailed {
            operation: "committing install state",
            source: source.into(),
        })
}

/// Mark a package as failed.
///
/// The outer install flow uses this during rollback to preserve the failure
/// state in the local database after partial installation has been cleaned up.
pub fn mark_failed(conn: &crate::storage::DbConnection, name: &str) -> Result<()> {
    storage::update_status(conn, name, PackageStatus::Failed).map_err(|source| {
        InstallStateError::DatabaseOperationFailed {
            operation: "marking package as failed",
            source,
        }
    })
}

fn installing_package(
    name: impl Into<String>,
    version: impl Into<String>,
    kind: InstallerType,
    engine_kind: EngineKind,
    install_dir: &Path,
) -> Package {
    Package {
        name: name.into(),
        version: version.into(),
        kind,
        engine_kind,
        engine_metadata: None,
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies: Vec::new(),
        status: PackageStatus::Installing,
        // Provisional value for in-progress installs; mark_ok overwrites it with the final timestamp.
        installed_at: now(),
    }
}

fn msi_inventory_receipt(
    package_name: &str,
    engine_metadata: Option<&EngineMetadata>,
) -> Option<MsiInventoryReceipt> {
    match engine_metadata? {
        EngineMetadata::Msi {
            product_code,
            upgrade_code,
            scope,
            ..
        } => Some(MsiInventoryReceipt {
            package_name: package_name.to_string(),
            product_code: product_code.clone(),
            upgrade_code: upgrade_code.clone(),
            scope: *scope,
        }),
        EngineMetadata::Msix { .. } => None,
    }
}
