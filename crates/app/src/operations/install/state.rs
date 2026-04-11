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
use winbrew_models::{EngineKind, InstallerType, Package, PackageStatus};

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
        // Provisional value for in-progress installs; storage::commit_install overwrites it.
        installed_at: now(),
    }
}

#[cfg(test)]
mod tests {
    use crate::core::paths::resolved_paths;
    use crate::storage;
    use std::path::Path;
    use tempfile::tempdir;
    use winbrew_models::{
        EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope, InstallerType,
        MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
        MsiRegistryRecord, MsiShortcutRecord, Package, PackageStatus,
    };

    fn init_storage(root: &Path) {
        let packages = root.join("packages").to_string_lossy().into_owned();
        let data = root.join("data").to_string_lossy().into_owned();
        let logs = root.join("logs").to_string_lossy().into_owned();
        let cache = root.join("cache").to_string_lossy().into_owned();
        let paths = resolved_paths(root, &packages, &data, &logs, &cache);

        storage::init(&paths).expect("storage should initialize");
    }

    fn sample_package(name: &str, install_dir: &str) -> Package {
        Package {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Msi,
            engine_kind: EngineKind::Msi,
            engine_metadata: None,
            install_dir: install_dir.to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Installing,
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        }
    }

    fn sample_snapshot(name: &str, install_dir: &str) -> MsiInventorySnapshot {
        let normalized_install_dir = install_dir.replace('\\', "/").to_ascii_lowercase();

        MsiInventorySnapshot {
            receipt: MsiInventoryReceipt {
                package_name: name.to_string(),
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: Some("{22222222-2222-2222-2222-222222222222}".to_string()),
                scope: InstallScope::Installed,
            },
            files: vec![MsiFileRecord {
                package_name: name.to_string(),
                path: format!("{install_dir}/bin/demo.exe"),
                normalized_path: format!("{normalized_install_dir}/bin/demo.exe"),
                hash_algorithm: None,
                hash_hex: None,
                is_config_file: false,
            }],
            registry_entries: vec![MsiRegistryRecord {
                package_name: name.to_string(),
                hive: "HKLM".to_string(),
                key_path: "Software\\Demo".to_string(),
                normalized_key_path: "software\\demo".to_string(),
                value_name: "InstallPath".to_string(),
                value_data: Some(install_dir.to_string()),
                previous_value: None,
            }],
            shortcuts: vec![MsiShortcutRecord {
                package_name: name.to_string(),
                path: format!("{install_dir}/Desktop/Demo.lnk"),
                normalized_path: format!("{normalized_install_dir}/desktop/demo.lnk"),
                target_path: Some(format!("{install_dir}/bin/demo.exe")),
                normalized_target_path: Some(format!("{normalized_install_dir}/bin/demo.exe")),
            }],
            components: vec![MsiComponentRecord {
                package_name: name.to_string(),
                component_id: "COMPONENT-DEMO".to_string(),
                path: Some(format!("{install_dir}/bin/demo.exe")),
                normalized_path: Some(format!("{normalized_install_dir}/bin/demo.exe")),
            }],
        }
    }

    #[test]
    fn commit_install_persists_msi_snapshot_transactionally() {
        let root = tempdir().expect("temp root");
        init_storage(root.path());

        let package_name = "demo";
        let install_dir = "C:/Tools/Actual";

        let conn = storage::get_conn().expect("database connection should open");
        storage::insert_package(&conn, &sample_package(package_name, "C:/Tools/Old"))
            .expect("insert package");

        let mut receipt = EngineInstallReceipt::new(
            EngineKind::Msi,
            install_dir.to_string(),
            Some(EngineMetadata::Msi {
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: Some("{22222222-2222-2222-2222-222222222222}".to_string()),
                scope: InstallScope::Installed,
                registry_keys: vec!["HKLM\\Software\\Demo".to_string()],
                shortcuts: vec!["C:/Users/Public/Desktop/Demo.lnk".to_string()],
            }),
        );
        receipt.msi_inventory_snapshot = Some(sample_snapshot(package_name, install_dir));

        let mut conn = conn;
        storage::commit_install(&mut conn, package_name, &receipt).expect("commit package install");

        let package = storage::get_package(&conn, package_name)
            .expect("read package")
            .expect("package should exist");

        assert_eq!(package.status, PackageStatus::Ok);
        assert_eq!(package.install_dir, install_dir);

        let file_owners =
            storage::find_packages_by_normalized_path(&conn, "c:/tools/actual/bin/demo.exe")
                .expect("lookup file owners");
        assert_eq!(file_owners, vec![package_name.to_string()]);

        let registry_owners =
            storage::find_packages_by_normalized_registry_key_path(&conn, "software\\demo")
                .expect("lookup registry owners");
        assert_eq!(registry_owners, vec![package_name.to_string()]);
    }
}
