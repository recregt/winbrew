use anyhow::{Context, Result};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

use super::uninstall::UNINSTALL;

/// Guard for a temporary uninstall registry entry used by tests.
#[doc(hidden)]
pub struct UninstallEntryGuard {
    root: RegKey,
    key_name: String,
}

/// Create a temporary uninstall registry entry under HKCU for tests.
#[doc(hidden)]
pub fn create_test_uninstall_entry(
    package_name: &str,
    install_dir: &Path,
    quiet_uninstall_command: Option<&str>,
    uninstall_command: Option<&str>,
) -> Result<UninstallEntryGuard> {
    create_test_uninstall_entry_with_install_location(
        package_name,
        Some(install_dir),
        quiet_uninstall_command,
        uninstall_command,
    )
}

/// Create a temporary uninstall registry entry under HKCU for tests.
#[doc(hidden)]
pub fn create_test_uninstall_entry_with_install_location(
    package_name: &str,
    install_location: Option<&Path>,
    quiet_uninstall_command: Option<&str>,
    uninstall_command: Option<&str>,
) -> Result<UninstallEntryGuard> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (root, _) = hkcu
        .create_subkey(UNINSTALL)
        .context("failed to create test uninstall root")?;

    let key_name = format!(
        "WinBrew.NativeExe.Test.{}.{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time is before UNIX_EPOCH")?
            .as_nanos()
    );

    {
        let (app_key, _) = root
            .create_subkey(&key_name)
            .context("failed to create test uninstall entry")?;

        let display_name = package_name.to_string();
        app_key
            .set_value("DisplayName", &display_name)
            .context("failed to set test uninstall display name")?;

        if let Some(install_location) = install_location {
            let install_location = install_location.to_string_lossy().to_string();
            app_key
                .set_value("InstallLocation", &install_location)
                .context("failed to set test uninstall install location")?;
        }

        if let Some(command) = quiet_uninstall_command {
            let quiet_command = command.to_string();
            app_key
                .set_value("QuietUninstallString", &quiet_command)
                .context("failed to set test quiet uninstall command")?;
        }

        if let Some(command) = uninstall_command {
            let uninstall_command = command.to_string();
            app_key
                .set_value("UninstallString", &uninstall_command)
                .context("failed to set test uninstall command")?;
        }
    }

    Ok(UninstallEntryGuard { root, key_name })
}

impl Drop for UninstallEntryGuard {
    fn drop(&mut self) {
        let _ = self.root.delete_subkey_all(&self.key_name);
    }
}
