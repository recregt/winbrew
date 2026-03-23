use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

use std::path::PathBuf;
use std::process::Command;

use crate::database;

pub fn find_dependents(name: &str, conn: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut dependents = database::list_packages(conn)?
        .into_iter()
        .filter(|pkg| {
            pkg.name != name
                && pkg
                    .dependencies
                    .iter()
                    .any(|dep| dependency_name(dep).eq_ignore_ascii_case(name))
        })
        .map(|pkg| pkg.name)
        .collect::<Vec<_>>();

    dependents.sort_unstable();
    dependents.dedup();

    Ok(dependents)
}

pub fn remove(name: &str, force: bool) -> Result<()> {
    debug!(package = name, force, "starting remove");

    let conn = database::lock_conn()?;

    remove_with_conn(name, force, &conn)
}

fn remove_with_conn(name: &str, force: bool, conn: &rusqlite::Connection) -> Result<()> {
    if !force {
        let dependents = find_dependents(name, conn)?;
        if !dependents.is_empty() {
            bail!(
                "cannot remove '{name}' because it is required by: {}",
                dependents.join(", ")
            );
        }
    }

    let pkg = database::get_package(conn, name)?.context(format!("{} is not installed", name))?;

    let install_dir = PathBuf::from(&pkg.install_dir);

    if pkg.kind.eq_ignore_ascii_case("msi") {
        let product_code = pkg
            .product_code
            .as_deref()
            .context("missing MSI product code in package record")?;

        let status = Command::new("msiexec")
            .args(["/x", product_code])
            .status()
            .context("failed to start msiexec")?;

        if !status.success() {
            bail!("msi uninstall failed with code: {:?}", status.code());
        }

        if install_dir.exists() {
            if let Err(err) = std::fs::remove_dir_all(&install_dir) {
                warn!("failed to remove package directory for {name}: {err}");
            }
        }

        database::delete_package(conn, name)?;
        debug!(package = name, force, "remove completed");
        return Ok(());
    }

    if install_dir.exists() {
        let trash_dir = install_dir.with_extension("trash");

        if trash_dir.exists() {
            std::fs::remove_dir_all(&trash_dir)
                .context("failed to clean up old trash directory")?;
        }

        std::fs::rename(&install_dir, &trash_dir).context("failed to stage package for removal")?;

        if let Err(err) = database::delete_package(conn, name) {
            let _ = std::fs::rename(&trash_dir, &install_dir);
            return Err(err).context("failed to remove package from database");
        }

        if let Err(err) = std::fs::remove_dir_all(&trash_dir) {
            warn!("failed to completely remove trash for {name}: {err}");
        }
    } else {
        database::delete_package(conn, name)?;
    }

    debug!(package = name, force, "remove completed");

    Ok(())
}

fn dependency_name(dep: &str) -> &str {
    dep.split_once('@').map(|(name, _)| name).unwrap_or(dep)
}
