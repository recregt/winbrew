pub mod install;
pub mod remove;

use anyhow::{Context, Result};
use windows::Management::Deployment::PackageManager;

/// Resolve the installed full package name for an MSIX package name.
///
/// The lookup accepts either a package full name or a package family name.
/// If exactly one installed package matches, its full name is returned.
/// Zero matches and ambiguous matches both return an error so the caller can
/// handle the mismatch explicitly.
pub fn installed_package_full_name(package_name: &str) -> Result<String> {
    let package_manager = PackageManager::new().context("failed to create package manager")?;
    let matching_full_names = remove::matching_package_full_names(&package_manager, package_name)?;

    match matching_full_names.as_slice() {
        [full_name] => Ok(full_name.to_string()),
        [] => anyhow::bail!("no installed msix package matched '{package_name}'"),
        _ => anyhow::bail!("multiple installed msix packages matched '{package_name}'"),
    }
}
