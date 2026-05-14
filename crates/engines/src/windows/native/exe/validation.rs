use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn validate_install_inputs(
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<()> {
    validate_download_path(download_path)?;
    validate_install_dir(install_dir)?;
    validate_package_name(package_name)?;

    Ok(())
}

pub(super) fn validate_download_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        bail!("installer path cannot be empty");
    }

    if !path.exists() {
        bail!("installer path does not exist: {}", path.display());
    }

    if !path.is_file() {
        bail!("installer path is not a file: {}", path.display());
    }

    Ok(())
}

pub(super) fn validate_install_dir(path: &Path) -> Result<()> {
    let path_text = path.to_string_lossy();

    if path.as_os_str().is_empty() || path_text.trim().is_empty() {
        bail!("install directory cannot be empty");
    }

    Ok(())
}

pub(super) fn validate_package_name(package_name: &str) -> Result<()> {
    let package_name = package_name.trim();

    if package_name.is_empty() {
        bail!("package name cannot be empty");
    }

    if package_name.chars().any(|ch| ch.is_control()) {
        bail!("package name contains invalid control characters");
    }

    Ok(())
}
