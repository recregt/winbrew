use chrono::Utc;
use std::path::Path;
use thiserror::Error;

use crate::core::fs::cleanup_path;
use crate::storage;
use winbrew_models::{InstallerType, Package, PackageStatus};

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

pub type Result<T> = std::result::Result<T, InstallStateError>;

pub fn prepare_install_target(
    conn: &rusqlite::Connection,
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

pub fn mark_installing(
    conn: &rusqlite::Connection,
    name: impl Into<String>,
    version: impl Into<String>,
    kind: InstallerType,
    install_dir: &Path,
) -> Result<()> {
    let package = installing_package(name, version, kind, install_dir);
    storage::insert_package(conn, &package).map_err(|source| {
        InstallStateError::DatabaseOperationFailed {
            operation: "marking package as installing",
            source,
        }
    })
}

pub fn mark_ok(
    conn: &rusqlite::Connection,
    name: &str,
    msix_package_full_name: Option<&str>,
) -> Result<()> {
    storage::update_status_and_msix_package_full_name(
        conn,
        name,
        PackageStatus::Ok,
        msix_package_full_name,
    )
    .map_err(|source| InstallStateError::DatabaseOperationFailed {
        operation: "marking package as installed",
        source,
    })
}

pub fn mark_failed(conn: &rusqlite::Connection, name: &str) -> Result<()> {
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
    install_dir: &Path,
) -> Package {
    Package {
        name: name.into(),
        version: version.into(),
        kind,
        install_dir: install_dir.to_string_lossy().into_owned(),
        msix_package_full_name: None,
        dependencies: Vec::new(),
        status: PackageStatus::Installing,
        installed_at: Utc::now().to_rfc3339(),
    }
}
