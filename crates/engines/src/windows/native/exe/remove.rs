use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;
use tracing::warn;

use crate::core::fs::cleanup_path;
use crate::models::install::installed::InstalledPackage;

use super::NATIVE_EXE_SUCCESS_EXIT_CODES;
use super::switches::split_switches;
use super::validation::{validate_install_dir, validate_package_name};

/// Remove a native executable package.
///
/// The backend prefers the recorded uninstall command from
/// `EngineMetadata::NativeExe` when one is available. If the uninstall command
/// fails or is missing, the module falls back to direct directory cleanup so the
/// install tree is still removed.
pub fn remove(package: &InstalledPackage) -> Result<()> {
    validate_package_name(&package.name)?;
    validate_install_dir(Path::new(&package.install_dir))?;

    let uninstall_command = package
        .engine_metadata
        .as_ref()
        .and_then(|metadata| metadata.native_exe_uninstall_command());

    if let Some(command) = uninstall_command {
        if let Err(err) = run_uninstall_command(command, &package.name) {
            warn!(
                package = package.name.as_str(),
                error = %err,
                "native executable uninstall command failed; falling back to directory cleanup"
            );
        }
    } else {
        warn!(
            package = package.name.as_str(),
            install_dir = %package.install_dir,
            "native executable uninstall metadata was not available; falling back to directory cleanup"
        );
    }

    cleanup_path(Path::new(&package.install_dir))
        .with_context(|| format!("failed to remove {}", package.install_dir))?;

    Ok(())
}

fn run_uninstall_command(command: &str, package_name: &str) -> Result<()> {
    let mut command_parts = split_switches(command)?;

    if command_parts.is_empty() {
        bail!("native executable uninstall command is empty for '{package_name}'");
    }

    let program = command_parts.remove(0);
    let status = Command::new(program)
        .args(command_parts)
        .status()
        .with_context(|| {
            format!("failed to launch native executable uninstaller for {package_name}")
        })?;

    let exit_code = status.code().ok_or_else(|| {
        anyhow::anyhow!("native executable uninstaller terminated without an exit code")
    })?;

    if !NATIVE_EXE_SUCCESS_EXIT_CODES.contains(&exit_code) {
        bail!(
            "native executable uninstaller for {} failed with exit code {}",
            package_name,
            exit_code
        );
    }

    Ok(())
}
