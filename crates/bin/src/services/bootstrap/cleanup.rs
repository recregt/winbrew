//! Bootstrap-only cleanup for incomplete installs.
//!
//! This module mutates install state during startup to recover from interrupted
//! installs. It is not a general-purpose service and should remain bootstrap-only.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::core::fs::cleanup_path;
use crate::models::{Package, PackageStatus};
use crate::services::shared::storage;
use crate::services::shared::temp_workspace;

pub fn cleanup_stale_installations() -> Result<()> {
    let conn = storage::get_conn()?;
    let stale_packages = storage::list_installing_packages(&conn)?;

    for package in stale_packages {
        cleanup_stale_installation(&conn, &package);
    }

    Ok(())
}

fn cleanup_stale_installation(conn: &crate::database::DbConnection, package: &Package) {
    if let Err(err) = storage::update_status(conn, &package.name, PackageStatus::Failed) {
        warn!(package = %package.name, error = %err, "failed to mark stale install as failed");
    }

    let install_dir = PathBuf::from(&package.install_dir);
    cleanup_install_dir(&install_dir, &package.name);
    cleanup_temp_roots(&package.name, &package.version);
}

fn cleanup_install_dir(install_dir: &Path, package_name: &str) {
    if let Err(err) = cleanup_path(install_dir) {
        warn!(package = package_name, path = %install_dir.display(), error = %err, "failed to clean stale install directory");
    }
}

fn cleanup_temp_roots(name: &str, version: &str) {
    let prefix = temp_workspace::temp_root_prefix(name, version);
    let temp_root_base = temp_workspace::temp_root_base();

    if !temp_root_base.exists() {
        return;
    }

    let entries = match fs::read_dir(&temp_root_base) {
        Ok(entries) => entries,
        Err(err) => {
            warn!(package = name, error = %err, "failed to enumerate temp directory for stale install cleanup");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|value| value.to_str()) {
            Some(file_name) => file_name,
            None => continue,
        };

        if file_name.starts_with(&prefix)
            && let Err(err) = cleanup_path(&path)
        {
            warn!(package = name, path = %path.display(), error = %err, "failed to clean stale temp root");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cleanup_stale_installations;
    use crate::models::{InstallerType, Package, PackageStatus};
    use crate::services::shared::storage;
    use crate::services::shared::temp_workspace;
    use std::fs;
    use tempfile::tempdir;

    fn sample_package(name: &str, version: &str, install_dir: &std::path::Path) -> Package {
        Package {
            name: name.to_string(),
            version: version.to_string(),
            kind: InstallerType::Portable,
            install_dir: install_dir.to_string_lossy().into_owned(),
            msix_package_full_name: None,
            dependencies: Vec::new(),
            status: PackageStatus::Installing,
            installed_at: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn cleanup_stale_installations_marks_installing_packages_failed_and_cleans_artifacts() {
        let temp_root = tempdir().expect("temp root");
        let root = temp_root.path();

        let config = crate::database::Config::load_at(root).expect("config should load");
        storage::init(&config.resolved_paths()).expect("database should initialize");

        let conn = storage::get_conn().expect("db connection");
        let install_dir = root.join("packages").join("Contoso.Stale");
        fs::create_dir_all(&install_dir).expect("install dir");

        let package = sample_package("Contoso.Stale", "1.0.0", &install_dir);
        storage::insert_package(&conn, &package).expect("insert package");

        let temp_root_path = temp_workspace::temp_root_base().join(format!(
            "{}test",
            temp_workspace::temp_root_prefix(&package.name, &package.version)
        ));
        fs::create_dir_all(&temp_root_path).expect("stale temp root");

        cleanup_stale_installations().expect("cleanup should succeed");

        let stored = storage::get_package(&conn, &package.name)
            .expect("query package")
            .expect("package should still exist");
        assert_eq!(stored.status, PackageStatus::Failed);
        assert!(!install_dir.exists());
        assert!(!temp_root_path.exists());
    }
}
