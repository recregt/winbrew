use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::database;

/// Publish command shims for the given installed package.
///
/// The shim files are created under the managed `shims/` root so the caller
/// can keep a single PATH entry instead of exposing package install roots.
pub fn publish_package_shims(
    shims_root: &Path,
    package_name: &str,
    bin_metadata: Option<&str>,
) -> Result<usize> {
    let conn = database::get_conn()?;
    let package = database::get_package(&conn, package_name)?.with_context(|| {
        format!("package '{package_name}' was not found while publishing shims")
    })?;
    let commands = database::list_commands_for_package(&conn, package_name)?;
    let target_paths = parse_target_paths(bin_metadata)?;

    publish_shims_for_install_dir(
        shims_root,
        Path::new(&package.install_dir),
        &commands,
        &target_paths,
    )
}

/// Publish command shims for the given install directory and command list.
pub fn publish_shims_for_install_dir(
    shims_root: &Path,
    install_dir: &Path,
    commands: &[String],
    target_paths: &[String],
) -> Result<usize> {
    if commands.is_empty() {
        return Ok(0);
    }

    fs::create_dir_all(shims_root)
        .with_context(|| format!("failed to create {}", shims_root.display()))?;

    let mut written = 0usize;

    for (index, command) in commands.iter().enumerate() {
        let shim_path = command_shim_path(shims_root, command);
        let target_path = resolve_target_path(index, target_paths);
        write_command_shim(&shim_path, install_dir, command, target_path.as_deref())?;
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

fn write_command_shim(
    path: &Path,
    install_dir: &Path,
    command_name: &str,
    target_path: Option<&str>,
) -> Result<()> {
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

    let script = command_shim_script(install_dir, command_name, target_path);
    file.write_all(script.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

fn command_shim_script(
    install_dir: &Path,
    command_name: &str,
    target_path: Option<&str>,
) -> String {
    match target_path {
        Some(target_path) => explicit_command_shim_script(install_dir, command_name, target_path),
        None => legacy_command_shim_script(install_dir, command_name),
    }
}

fn explicit_command_shim_script(
    install_dir: &Path,
    command_name: &str,
    target_path: &str,
) -> String {
    let install_dir = install_dir.to_string_lossy();
    let target_path = normalize_path_separators(target_path);
    let target_extension = Path::new(&target_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let invocation = if matches!(target_extension.as_str(), "cmd" | "bat") {
        "call \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_TARGET%\" %*".to_string()
    } else {
        "\"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_TARGET%\" %*".to_string()
    };

    format!(
        "@echo off\r\nsetlocal\r\nset \"WINBREW_PACKAGE_DIR={install_dir}\"\r\nset \"WINBREW_SHIM_TARGET={target_path}\"\r\nif exist \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_TARGET%\" (\r\n  {invocation}\r\n  exit /b %ERRORLEVEL%\r\n)\r\necho WinBrew shim for {command_name} could not find target executable at %WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_TARGET%.\r\nexit /b 1\r\n",
    )
}

fn legacy_command_shim_script(install_dir: &Path, command_name: &str) -> String {
    let install_dir = install_dir.to_string_lossy();

    format!(
        "@echo off\r\nsetlocal\r\nset \"WINBREW_PACKAGE_DIR={install_dir}\"\r\nset \"WINBREW_SHIM_NAME=%~n0\"\r\nif exist \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.exe\" (\r\n  \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.exe\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\nif exist \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.exe\" (\r\n  \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.exe\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\nif exist \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.cmd\" (\r\n  call \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_NAME%.cmd\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\nif exist \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.cmd\" (\r\n  call \"%WINBREW_PACKAGE_DIR%\\bin\\%WINBREW_SHIM_NAME%.cmd\" %*\r\n  exit /b %ERRORLEVEL%\r\n)\r\necho WinBrew shim for {command_name} could not find a target executable.\r\nexit /b 1\r\n",
    )
}

fn parse_target_paths(raw_targets: Option<&str>) -> Result<Vec<String>> {
    let Some(raw_targets) = raw_targets else {
        return Ok(Vec::new());
    };

    let targets: Vec<String> =
        serde_json::from_str(raw_targets).with_context(|| "failed to parse shim target JSON")?;

    Ok(normalize_target_paths(targets))
}

fn normalize_target_paths<I, S>(targets: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for target in targets {
        let normalized_target = normalize_path_separators(target.as_ref().trim());
        if normalized_target.is_empty() {
            continue;
        }

        let dedupe_key = normalized_target.to_ascii_lowercase();
        if seen.insert(dedupe_key) {
            normalized.push(normalized_target);
        }
    }

    normalized
}

fn normalize_path_separators(path: &str) -> String {
    path.replace('/', "\\")
}

fn resolve_target_path(index: usize, target_paths: &[String]) -> Option<String> {
    target_paths
        .get(index)
        .or_else(|| target_paths.first())
        .map(|target_path| target_path.to_string())
}
