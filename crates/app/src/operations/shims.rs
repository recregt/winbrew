use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::database;

/// Publish command shims for the given installed package.
///
/// The shim files are created under the managed `shims/` root so the caller
/// can keep a single PATH entry instead of exposing package install roots.
pub fn publish_package_shims(shims_root: &Path, package_name: &str) -> Result<usize> {
    let conn = database::get_conn()?;
    let package = database::get_package(&conn, package_name)?.with_context(|| {
        format!("package '{package_name}' was not found while publishing shims")
    })?;
    let commands = database::list_commands_for_package(&conn, package_name)?;

    publish_shims_for_install_dir(shims_root, Path::new(&package.install_dir), &commands)
}

/// Publish command shims for the given install directory and command list.
pub fn publish_shims_for_install_dir(
    shims_root: &Path,
    install_dir: &Path,
    commands: &[String],
) -> Result<usize> {
    if commands.is_empty() {
        return Ok(0);
    }

    fs::create_dir_all(shims_root)
        .with_context(|| format!("failed to create {}", shims_root.display()))?;

    let mut written = 0usize;

    for command in commands {
        let shim_path = command_shim_path(shims_root, command);
        write_command_shim(&shim_path, install_dir, command)?;
        written += 1;
    }

    Ok(written)
}

/// Remove command shims for the given command list.
pub fn remove_shim_files(shims_root: &Path, commands: &[String]) -> Result<usize> {
    let mut removed = 0usize;

    for command in commands {
        let shim_path = command_shim_path(shims_root, command);
        match fs::remove_file(&shim_path) {
            Ok(()) => removed += 1,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to remove shim {}", shim_path.display()));
            }
        }
    }

    Ok(removed)
}

/// Return the on-disk path for a command shim under the managed `shims/` root.
pub fn command_shim_path(shims_root: &Path, command_name: &str) -> PathBuf {
    shims_root.join(format!("{command_name}.cmd"))
}

fn write_command_shim(path: &Path, install_dir: &Path, command_name: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;

    let script = command_shim_script(install_dir, command_name);
    file.write_all(script.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

fn command_shim_script(install_dir: &Path, command_name: &str) -> String {
    let install_dir = install_dir.to_string_lossy();

    format!(
        "@echo off\r\nsetlocal\r\nset \"WINBREW_PACKAGE_DIR={install_dir}\"\r\nset \"WINBREW_SHIM_NAME=%~n0\"\r\nif exist \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.exe\" (\r\n  \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.exe\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\nif exist \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.exe\" (\r\n  \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.exe\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\nif exist \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.cmd\" (\r\n  call \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.cmd\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\nif exist \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.cmd\" (\r\n  call \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.cmd\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\necho WinBrew shim for {command_name} could not find a target executable.\r\nexit /b 1\r\n",
    )
}
