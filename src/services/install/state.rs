use anyhow::{Result, bail};
use chrono::Utc;
use std::path::Path;

use crate::database;
use crate::models::{Package, PackageStatus};

use super::staging;

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
                let _ = database::delete_package(conn, name);
                staging::cleanup_path(install_dir)?;
            }
        }
    } else if install_dir.exists() {
        staging::cleanup_path(install_dir)?;
    }

    Ok(())
}

pub fn mark_installing(
    conn: &rusqlite::Connection,
    name: &str,
    version: &str,
    kind: &str,
    install_dir: &Path,
) -> Result<()> {
    database::insert_package(
        conn,
        &Package {
            name: name.to_string(),
            version: version.to_string(),
            kind: kind.to_string(),
            install_dir: install_dir.to_string_lossy().to_string(),
            product_code: None,
            dependencies: Vec::new(),
            status: PackageStatus::Installing,
            installed_at: Utc::now().to_rfc3339(),
        },
    )
}

pub fn mark_ok(conn: &rusqlite::Connection, name: &str) -> Result<()> {
    database::update_status(conn, name, PackageStatus::Ok)
}

pub fn mark_failed(conn: &rusqlite::Connection, name: &str) -> Result<()> {
    database::update_status(conn, name, PackageStatus::Failed)
}
