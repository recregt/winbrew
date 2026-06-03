use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::database;

#[derive(Debug, Clone)]
pub(crate) struct ShimTarget {
    alias: Option<String>,
    target_path: String,
    default_args: Vec<String>,
}

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
    let targets = parse_shim_targets(bin_metadata)?;

    publish_shims_for_install_dir(
        shims_root,
        Path::new(&package.install_dir),
        &commands,
        &targets,
    )
}

/// Publish command shims for the given install directory and command list.
pub fn publish_shims_for_install_dir(
    shims_root: &Path,
    install_dir: &Path,
    commands: &[String],
    targets: &[ShimTarget],
) -> Result<usize> {
    if commands.is_empty() {
        return Ok(0);
    }

    fs::create_dir_all(shims_root)
        .with_context(|| format!("failed to create {}", shims_root.display()))?;

    let mut written = 0usize;
    let mut alias_lookup = BTreeMap::new();

    for (index, target) in targets.iter().enumerate() {
        if let Some(alias) = target.alias.as_deref() {
            alias_lookup
                .entry(normalize_command_name(alias))
                .or_insert(index);
        }
    }

    for (index, command) in commands.iter().enumerate() {
        let shim_path = command_shim_path(shims_root, command);
        let target = alias_lookup
            .get(&normalize_command_name(command))
            .and_then(|index| targets.get(*index))
            .or_else(|| targets.get(index));
        write_command_shim(&shim_path, install_dir, command, target)?;
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
    target: Option<&ShimTarget>,
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

    let script = command_shim_script(install_dir, command_name, target);
    file.write_all(script.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

fn command_shim_script(
    install_dir: &Path,
    command_name: &str,
    target: Option<&ShimTarget>,
) -> String {
    match target {
        Some(target) => explicit_command_shim_script(
            install_dir,
            command_name,
            &target.target_path,
            &target.default_args,
        ),
        None => legacy_command_shim_script(install_dir, command_name),
    }
}

fn explicit_command_shim_script(
    install_dir: &Path,
    command_name: &str,
    target_path: &str,
    default_args: &[String],
) -> String {
    let install_dir = install_dir.to_string_lossy();
    let target_path = normalize_path_separators(target_path);
    let default_args = render_default_args(default_args);
    let target_extension = Path::new(&target_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let invocation = if matches!(target_extension.as_str(), "cmd" | "bat") {
        format!("call \"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_TARGET%\"{default_args} %*")
    } else {
        format!("\"%WINBREW_PACKAGE_DIR%\\%WINBREW_SHIM_TARGET%\"{default_args} %*")
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

pub(crate) fn parse_target_paths(raw_targets: Option<&str>) -> Result<Vec<String>> {
    let targets = parse_shim_targets(raw_targets)?;
    Ok(normalize_target_paths(
        targets.iter().map(|target| target.target_path.as_str()),
    ))
}

pub(crate) fn parse_journal_shim_bindings(
    raw_targets: Option<&str>,
) -> Result<Vec<database::JournalShimBinding>> {
    Ok(parse_shim_targets(raw_targets)?
        .into_iter()
        .map(journal_shim_binding_from_target)
        .collect())
}

pub(crate) fn target_paths_from_journal_bindings(
    bindings: &[database::JournalShimBinding],
) -> Vec<String> {
    normalize_target_paths(bindings.iter().map(|binding| binding.target_path.as_str()))
}

pub(crate) fn shim_targets_from_journal_bindings(
    bindings: &[database::JournalShimBinding],
) -> Vec<ShimTarget> {
    bindings
        .iter()
        .filter_map(|binding| {
            let target_path = normalize_path_separators(binding.target_path.trim());
            if target_path.is_empty() {
                return None;
            }

            Some(ShimTarget {
                alias: binding
                    .alias
                    .as_deref()
                    .map(str::trim)
                    .filter(|alias| !alias.is_empty())
                    .map(str::to_owned),
                target_path,
                default_args: binding
                    .default_args
                    .iter()
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
                    .collect(),
            })
        })
        .collect()
}

fn parse_shim_targets(raw_targets: Option<&str>) -> Result<Vec<ShimTarget>> {
    let Some(raw_targets) = raw_targets else {
        return Ok(Vec::new());
    };

    let raw_targets = serde_json::from_str::<serde_json::Value>(raw_targets)
        .with_context(|| "failed to parse shim target JSON")?;

    let targets = match raw_targets {
        serde_json::Value::String(target_path) => vec![parse_shim_target_string(target_path)],
        serde_json::Value::Array(values) => parse_shim_target_array(values)?,
        _ => {
            return Err(anyhow::anyhow!(
                "failed to parse shim target JSON: expected a string or array"
            ));
        }
    };

    Ok(targets)
}

pub(crate) fn legacy_shim_targets(target_paths: &[String]) -> Vec<ShimTarget> {
    target_paths
        .iter()
        .map(|target_path| ShimTarget {
            alias: None,
            target_path: normalize_path_separators(target_path.trim()),
            default_args: Vec::new(),
        })
        .collect()
}

fn parse_shim_target_array(values: Vec<serde_json::Value>) -> Result<Vec<ShimTarget>> {
    values.into_iter().map(parse_shim_target_value).collect()
}

fn parse_shim_target_value(value: serde_json::Value) -> Result<ShimTarget> {
    match value {
        serde_json::Value::String(target_path) => Ok(parse_shim_target_string(target_path)),
        serde_json::Value::Array(values) => parse_shim_tuple_target(values),
        _ => Err(anyhow::anyhow!(
            "failed to parse shim target JSON: expected string entries or array entries"
        )),
    }
}

fn parse_shim_tuple_target(values: Vec<serde_json::Value>) -> Result<ShimTarget> {
    let mut values = values.into_iter();
    let Some(target_path) = values
        .next()
        .and_then(|value| value.as_str().map(str::to_owned))
    else {
        return Err(anyhow::anyhow!(
            "failed to parse shim target JSON: expected target path as first tuple entry"
        ));
    };

    let alias = values
        .next()
        .and_then(|value| value.as_str().map(|value| value.trim().to_owned()))
        .filter(|value| !value.is_empty());

    let default_args = values
        .map(|value| {
            value
                .as_str()
                .map(|value| value.trim().to_owned())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "failed to parse shim target JSON: expected tuple args to be strings"
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect();

    Ok(ShimTarget {
        alias,
        target_path: normalize_path_separators(target_path.trim()),
        default_args,
    })
}

fn parse_shim_target_string(target_path: String) -> ShimTarget {
    ShimTarget {
        alias: None,
        target_path: normalize_path_separators(target_path.trim()),
        default_args: Vec::new(),
    }
}

fn journal_shim_binding_from_target(target: ShimTarget) -> database::JournalShimBinding {
    database::JournalShimBinding {
        alias: target.alias,
        target_path: target.target_path,
        default_args: target.default_args,
    }
}

fn normalize_command_name(command_name: &str) -> String {
    command_name.trim().to_ascii_lowercase()
}

fn render_default_args(default_args: &[String]) -> String {
    if default_args.is_empty() {
        return String::new();
    }

    let rendered = default_args
        .iter()
        .map(|argument| render_cmd_argument(argument))
        .collect::<Vec<_>>()
        .join(" ");

    format!(" {rendered}")
}

fn render_cmd_argument(argument: &str) -> String {
    if argument
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '"'))
    {
        format!("\"{}\"", argument.replace('"', "\"\""))
    } else {
        argument.to_owned()
    }
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

#[cfg(test)]
mod tests {
    use super::{command_shim_path, parse_target_paths, publish_package_shims};
    use crate::database;
    use crate::models::domains::install::InstallerType;
    use crate::models::domains::installed::{InstalledPackage, PackageStatus};
    use anyhow::Result;
    use std::fs;
    use std::path::Path;
    use winbrew_testing::{init_database, reset_install_state, test_root};

    fn sample_package(name: &str, kind: InstallerType, install_dir: &Path) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind,
            deployment_kind: kind.deployment_kind(),
            engine_kind: kind.into(),
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().into_owned(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn parse_target_paths_accepts_single_string_and_array() -> Result<()> {
        let single = parse_target_paths(Some(r#""bin/tool.exe""#))?;
        assert_eq!(single, vec!["bin\\tool.exe".to_string()]);

        let multiple = parse_target_paths(Some(r#"["bin/tool.exe", "bin/other.exe"]"#))?;
        assert_eq!(
            multiple,
            vec!["bin\\tool.exe".to_string(), "bin\\other.exe".to_string()]
        );

        Ok(())
    }

    #[test]
    fn publish_package_shims_accepts_single_string_bin_metadata() -> Result<()> {
        let test_root = test_root();
        let root = test_root.path();
        init_database(root)?;
        reset_install_state(root)?;
        let conn = database::get_conn()?;

        let install_dir = root.join("packages").join("Contoso.Shim");
        fs::create_dir_all(&install_dir)?;

        let package = sample_package("Contoso.Shim", InstallerType::Portable, &install_dir);
        database::insert_package(&conn, &package)?;
        database::sync_package_commands(&conn, &package.name, Some(r#"["contoso"]"#))?;

        let shims_root = root.join("shims");
        let written = publish_package_shims(&shims_root, &package.name, Some(r#""bin/tool.exe""#))?;

        assert_eq!(written, 1);

        let shim_path = command_shim_path(&shims_root, "contoso");
        assert!(shim_path.exists());

        let shim_contents = fs::read_to_string(shim_path)?;
        assert!(shim_contents.contains("WINBREW_SHIM_TARGET=bin\\tool.exe"));

        Ok(())
    }

    #[test]
    fn publish_package_shims_supports_tuple_bin_default_args() -> Result<()> {
        let test_root = test_root();
        let root = test_root.path();
        init_database(root)?;
        reset_install_state(root)?;
        let conn = database::get_conn()?;

        let install_dir = root.join("packages").join("Contoso.Tuple");
        fs::create_dir_all(&install_dir)?;

        let package = sample_package("Contoso.Tuple", InstallerType::Portable, &install_dir);
        database::insert_package(&conn, &package)?;
        database::sync_package_commands(&conn, &package.name, Some(r#"["git", "git-lfs"]"#))?;

        let shims_root = root.join("shims");
        let written = publish_package_shims(
            &shims_root,
            &package.name,
            Some(r#"[["bin/git.exe", "git", "--version"], ["bin/git-lfs.exe", "git-lfs"]]"#),
        )?;

        assert_eq!(written, 2);

        let git_shim = fs::read_to_string(command_shim_path(&shims_root, "git"))?;
        assert!(git_shim.contains("WINBREW_SHIM_TARGET=bin\\git.exe"));
        assert!(git_shim.contains("--version"));

        let git_lfs_shim = fs::read_to_string(command_shim_path(&shims_root, "git-lfs"))?;
        assert!(git_lfs_shim.contains("WINBREW_SHIM_TARGET=bin\\git-lfs.exe"));
        assert!(!git_lfs_shim.contains("--version"));

        Ok(())
    }
}
