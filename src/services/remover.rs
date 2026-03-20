use anyhow::{Context, Result};
use std::path::Path;
use std::path::PathBuf;

use crate::core::{paths, shim};
use crate::database;

pub fn find_dependents(name: &str) -> Result<Vec<String>> {
    let conn = database::lock_conn()?;

    let mut dependents = database::list_packages(&conn)?
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

pub fn remove(name: &str) -> Result<()> {
    let conn = database::lock_conn()?;

    let pkg = database::get_package(&conn, name)?.context(format!("{} is not installed", name))?;
    let install_root = paths::install_root_from_package_dir(Path::new(&pkg.install_dir));

    for s in &pkg.shims {
        shim::remove_at(&install_root, &s.name)?;
    }

    let install_dir = PathBuf::from(&pkg.install_dir);
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir).context("failed to remove package directory")?;
    }

    database::delete_package(&conn, name)?;

    Ok(())
}

fn dependency_name(dep: &str) -> &str {
    dep.rsplit_once('@').map(|(name, _)| name).unwrap_or(dep)
}
