use anyhow::{Result, bail};
use chrono::Utc;
use std::path::Path;

use crate::database;
use crate::engines::common::cleanup_path;
use crate::models::{Package, PackageStatus};

pub fn prepare_install_target(
    conn: &rusqlite::Connection,
    name: &str,
    install_dir: &Path,
) -> Result<()> {
    if let Some(existing) = database::get_package(conn, name)? {
        match existing.status {
            PackageStatus::Ok => bail!("package '{name}' is already installed"),
            PackageStatus::Installing => bail!("package '{name}' is already being installed"),
            PackageStatus::Updating => bail!("package '{name}' is currently updating"),
            PackageStatus::Failed => {
                database::delete_package(conn, name)?;
                cleanup_path(install_dir)?;
            }
        }
    } else if install_dir.exists() {
        cleanup_path(install_dir)?;
    }

    Ok(())
}

pub fn mark_installing(
    conn: &rusqlite::Connection,
    name: impl Into<String>,
    version: impl Into<String>,
    kind: impl Into<String>,
    install_dir: &Path,
) -> Result<()> {
    let package = installing_package(name, version, kind, install_dir);
    database::insert_package(conn, &package)
}

pub fn mark_ok(conn: &rusqlite::Connection, name: &str) -> Result<()> {
    database::update_status(conn, name, PackageStatus::Ok)
}

pub fn mark_failed(conn: &rusqlite::Connection, name: &str) -> Result<()> {
    database::update_status(conn, name, PackageStatus::Failed)
}

fn installing_package(
    name: impl Into<String>,
    version: impl Into<String>,
    kind: impl Into<String>,
    install_dir: &Path,
) -> Package {
    Package {
        name: name.into(),
        version: version.into(),
        kind: kind.into(),
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies: Vec::new(),
        status: PackageStatus::Installing,
        installed_at: Utc::now().to_rfc3339(),
    }
}
