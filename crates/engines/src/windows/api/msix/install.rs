//! MSIX installation implementation.
//!
//! This is a thin adapter around the Windows App Installer APIs exposed by
//! `crate::windows_dep::packages::msix_install`. It does not unpack files or manage a
//! portable install tree; it only records the MSIX receipt metadata WinBrew
//! needs later for removal.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::models::install::engine::{
    EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope,
};

use crate::windows_dep::packages::msix_install;

/// Install an MSIX package and return the receipt data WinBrew needs later.
///
/// The function calls into Windows to register the package, creates the target
/// install directory so the install record has a concrete path, and returns an
/// `EngineInstallReceipt` with `EngineKind::Msix` plus `EngineMetadata::Msix`.
pub fn install(
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    let package_full_name =
        msix_install(download_path, package_name).context("msix install failed")?;

    fs::create_dir_all(install_dir)
        .with_context(|| format!("failed to create {}", install_dir.display()))?;

    let engine_metadata = Some(EngineMetadata::msix(
        package_full_name,
        InstallScope::Installed,
    ));

    Ok(EngineInstallReceipt::new(
        EngineKind::Msix,
        install_dir.to_string_lossy().into_owned(),
        engine_metadata,
    ))
}
