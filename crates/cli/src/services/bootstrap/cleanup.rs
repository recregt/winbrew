//! Startup-only cleanup for interrupted installs.
//!
//! When the CLI starts, it needs to reconcile any package rows that were left
//! in the `Installing` state by a previous crash or forced termination. That
//! recovery is performed here, before command dispatch begins, so later command
//! handlers see a coherent view of the database and filesystem.
//!
//! The responsibilities in this module are intentionally narrow:
//!
//! - read the current set of installing packages from the database;
//! - mark each stale package as failed so it no longer looks active;
//! - remove the package's installation directory if it is still present;
//! - remove any temp-workspace directories that belong to the interrupted
//!   install.
//!
//! Nothing in this module is meant to be called as a general-purpose repair
//! API. It is a startup repair mechanism that depends on the database and the
//! core filesystem cleanup helpers already being available.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::core::fs::cleanup_path;
use crate::core::temp_workspace::{is_temp_root_for, temp_root_base};
use crate::database;
use crate::models::{Package, PackageStatus};

/// Find stale `Installing` rows and reconcile them with the filesystem.
/// The current database connection is obtained from the process-wide storage
/// layer, which means the caller must have already initialized the database for
/// the active configuration. Each stale package is handled independently so one
/// cleanup failure does not prevent the rest of the recovery pass from running.
pub fn cleanup_stale_installations() -> Result<()> {
    let conn = database::get_conn()?;
    let stale_packages = database::list_installing_packages(&conn)?;

    for package in stale_packages {
        cleanup_stale_installation(&conn, &package);
    }

    Ok(())
}

fn cleanup_stale_installation(conn: &crate::database::DbConnection, package: &Package) {
    if let Err(err) = database::update_status(conn, &package.name, PackageStatus::Failed) {
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
    let temp_root_base = temp_root_base();

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
        if is_temp_root_for(name, version, &path)
            && let Err(err) = cleanup_path(&path)
        {
            warn!(package = name, path = %path.display(), error = %err, "failed to clean stale temp root");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cleanup_stale_installations;
    use crate::core::temp_workspace;
    use crate::database;
    use crate::models::{InstallerType, Package, PackageStatus};
    use std::fs;
    use tempfile::tempdir;

    fn sample_package(name: &str, version: &str, install_dir: &std::path::Path) -> Package {
        Package {
            name: name.to_string(),
            version: version.to_string(),
            kind: InstallerType::Portable,
            engine_kind: InstallerType::Portable.into(),
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().into_owned(),
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
        database::init(&config.resolved_paths()).expect("database should initialize");

        let conn = database::get_conn().expect("db connection");
        let install_dir = root.join("packages").join("Contoso.Stale");
        fs::create_dir_all(&install_dir).expect("install dir");

        let package = sample_package("Contoso.Stale", "1.0.0", &install_dir);
        database::insert_package(&conn, &package).expect("insert package");

        let temp_root_path = temp_workspace::temp_root_base().join(format!(
            "{}test",
            temp_workspace::temp_root_prefix(&package.name, &package.version)
        ));
        fs::create_dir_all(&temp_root_path).expect("stale temp root");

        cleanup_stale_installations().expect("cleanup should succeed");

        let stored = database::get_package(&conn, &package.name)
            .expect("query package")
            .expect("package should still exist");
        assert_eq!(stored.status, PackageStatus::Failed);
        assert!(!install_dir.exists());
        assert!(!temp_root_path.exists());
    }
}
