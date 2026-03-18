use anyhow::{Context, Result};

use crate::core::{paths, shim};
use crate::database;

pub fn remove(name: &str) -> Result<()> {
    let conn = database::connect()?;

    let pkg = database::get_package(&conn, name)?.context(format!("{} is not installed", name))?;

    for s in &pkg.shims {
        shim::remove(&s.name)?;
    }

    let install_dir = paths::package_dir(name);
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir).context("failed to remove package directory")?;
    }

    database::delete_package(&conn, name)?;

    Ok(())
}
