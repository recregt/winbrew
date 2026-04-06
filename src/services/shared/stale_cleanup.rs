use anyhow::Result;
use std::fs;
use std::path::Path;
use tracing::warn;

use crate::core::fs::cleanup_path;
use crate::database;
use crate::models::Package;

use crate::services::app::install::{state, workspace};

pub fn cleanup_stale_installations() -> Result<()> {
    let conn = database::get_conn()?;
    let stale_packages = database::list_installing_packages(&conn)?;

    for package in stale_packages {
        cleanup_stale_installation(&conn, &package);
    }

    Ok(())
}

fn cleanup_stale_installation(conn: &rusqlite::Connection, package: &Package) {
    if let Err(err) = state::mark_failed(conn, &package.name) {
        warn!(package = %package.name, error = %err, "failed to mark stale install as failed");
    }

    cleanup_install_dir(Path::new(&package.install_dir), &package.name);
    cleanup_temp_roots(&package.name, &package.version);
}

fn cleanup_install_dir(install_dir: &Path, package_name: &str) {
    if let Err(err) = cleanup_path(install_dir) {
        warn!(package = package_name, path = %install_dir.display(), error = %err, "failed to clean stale install directory");
    }
}

fn cleanup_temp_roots(name: &str, version: &str) {
    let prefix = workspace::temp_root_prefix(name, version);
    let temp_root_base = workspace::temp_root_base();

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
    use crate::database;
    use crate::models::{Package, PackageStatus};
    use std::fs;
    use tempfile::tempdir;

    fn sample_package(name: &str, version: &str, install_dir: &std::path::Path) -> Package {
        Package {
            name: name.to_string(),
            version: version.to_string(),
            kind: "portable".to_string(),
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

        let config = database::Config::load_at(root).expect("config should load");
        database::init(&config.resolved_paths()).expect("database should initialize");

        let conn = database::get_conn().expect("db connection");
        let install_dir = root.join("packages").join("Contoso.Stale");
        fs::create_dir_all(&install_dir).expect("install dir");

        let package = sample_package("Contoso.Stale", "1.0.0", &install_dir);
        database::insert_package(&conn, &package).expect("insert package");

        let temp_root_path = std::env::temp_dir().join(format!(
            "{}test",
            super::workspace::temp_root_prefix(&package.name, &package.version)
        ));
        let temp_root_path = super::workspace::temp_root_base().join(
            temp_root_path
                .file_name()
                .expect("temp root should have a file name"),
        );
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
