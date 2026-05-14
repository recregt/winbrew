use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::warn;

use crate::models::catalog::package::CatalogInstaller;
use crate::models::install::engine::{EngineInstallReceipt, EngineKind, EngineMetadata};

use super::NATIVE_EXE_SUCCESS_EXIT_CODES;
use super::metadata::{NativeExeInstallMetadata, capture_native_exe_metadata};
use super::switches::build_install_args;
use super::validation::validate_install_inputs;

/// Install a native executable package by running the downloaded installer.
///
/// The installer family is expected to come from catalog metadata. The backend
/// validates the inputs, builds family-specific switches, executes the installer
/// process, and records uninstall metadata when Windows exposes it.
pub(crate) fn install(
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    validate_install_inputs(download_path, install_dir, package_name)?;

    fs::create_dir_all(install_dir)
        .with_context(|| format!("failed to create {}", install_dir.display()))?;

    let args = build_install_args(installer, install_dir, package_name)?;

    let status = Command::new(download_path)
        .current_dir(download_path.parent().unwrap_or(Path::new(".")))
        .args(&args)
        .status()
        .with_context(|| {
            format!("failed to launch native executable installer for {package_name}")
        })?;

    let exit_code = status.code().ok_or_else(|| {
        anyhow::anyhow!("native executable installer terminated without an exit code")
    })?;

    if !NATIVE_EXE_SUCCESS_EXIT_CODES.contains(&exit_code) {
        bail!(
            "native executable installer for {} failed with exit code {}",
            package_name,
            exit_code
        );
    }

    let captured_metadata = capture_native_exe_metadata(package_name, install_dir);

    if captured_metadata.is_none() {
        warn!(
            package = package_name,
            install_dir = %install_dir.display(),
            "native executable installer did not expose uninstall metadata"
        );
    }

    let engine_metadata = captured_metadata.map(|metadata| match metadata {
        NativeExeInstallMetadata::QuietOnly(command) => {
            EngineMetadata::native_exe(Some(command), None)
        }
        NativeExeInstallMetadata::QuietAndStandard {
            quiet_uninstall_command,
            uninstall_command,
        } => EngineMetadata::native_exe(Some(quiet_uninstall_command), Some(uninstall_command)),
        NativeExeInstallMetadata::StandardOnly(command) => {
            EngineMetadata::native_exe(None, Some(command))
        }
    });

    Ok(EngineInstallReceipt::new(
        EngineKind::NativeExe,
        install_dir.to_string_lossy().into_owned(),
        engine_metadata,
    ))
}
