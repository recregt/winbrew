use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::warn;

use winbrew_core::fs::cleanup_path;
use winbrew_models::catalog::package::CatalogInstaller;
use winbrew_models::install::engine::{EngineInstallReceipt, EngineKind, EngineMetadata};
use winbrew_models::install::installed::InstalledPackage;
use winbrew_models::install::installer::InstallerType;
use winbrew_windows::uninstall_roots;

const NATIVE_EXE_SUCCESS_EXIT_CODES: &[i32] = &[0, 1641, 3010];

pub fn install(
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    #[cfg(not(windows))]
    {
        let _ = (installer, download_path, install_dir, package_name);
        bail!("native executable installation is only supported on Windows")
    }

    #[cfg(windows)]
    {
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

        let engine_metadata =
            capture_native_exe_metadata(package_name, install_dir).map(|metadata| match metadata {
                NativeExeInstallMetadata::QuietOnly(command) => {
                    EngineMetadata::native_exe(Some(command), None)
                }
                NativeExeInstallMetadata::QuietAndStandard {
                    quiet_uninstall_command,
                    uninstall_command,
                } => EngineMetadata::native_exe(
                    Some(quiet_uninstall_command),
                    Some(uninstall_command),
                ),
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
}

pub fn remove(package: &InstalledPackage) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = package;
        bail!("native executable removal is only supported on Windows")
    }

    #[cfg(windows)]
    {
        if let Some(command) = package
            .engine_metadata
            .as_ref()
            .and_then(|metadata| metadata.native_exe_uninstall_command())
            && let Err(err) = run_uninstall_command(command, &package.name)
        {
            warn!(
                package = package.name.as_str(),
                error = %err,
                "native executable uninstall command failed; falling back to directory cleanup"
            );
        }

        cleanup_path(Path::new(&package.install_dir))
            .with_context(|| format!("failed to remove {}", package.install_dir))?;

        Ok(())
    }
}

enum NativeExeInstallMetadata {
    QuietOnly(String),
    QuietAndStandard {
        quiet_uninstall_command: String,
        uninstall_command: String,
    },
    StandardOnly(String),
}

fn capture_native_exe_metadata(
    package_name: &str,
    install_dir: &Path,
) -> Option<NativeExeInstallMetadata> {
    let mut fallback = None;
    let package_name = package_name.trim();

    for root in uninstall_roots() {
        for key_result in root.key.enum_keys() {
            let Ok(key_name) = key_result else { continue };
            let Ok(app_key) = root.key.open_subkey(&key_name) else {
                continue;
            };

            let Ok(display_name) = app_key.get_value::<String, _>("DisplayName") else {
                continue;
            };

            if !display_name.trim().eq_ignore_ascii_case(package_name) {
                continue;
            }

            let install_location = match app_key.get_value::<String, _>("InstallLocation") {
                Ok(value) if !value.trim().is_empty() => Some(value),
                _ => None,
            };

            if let Some(install_location) = install_location
                && !same_install_location(Path::new(&install_location), install_dir)
            {
                continue;
            }

            let quiet_uninstall_command =
                match app_key.get_value::<String, _>("QuietUninstallString") {
                    Ok(value) if !value.trim().is_empty() => Some(value),
                    _ => None,
                };
            let uninstall_command = match app_key.get_value::<String, _>("UninstallString") {
                Ok(value) if !value.trim().is_empty() => Some(value),
                _ => None,
            };

            match (quiet_uninstall_command, uninstall_command) {
                (Some(quiet_uninstall_command), Some(uninstall_command)) => {
                    return Some(NativeExeInstallMetadata::QuietAndStandard {
                        quiet_uninstall_command,
                        uninstall_command,
                    });
                }
                (Some(quiet_uninstall_command), None) => {
                    return Some(NativeExeInstallMetadata::QuietOnly(quiet_uninstall_command));
                }
                (None, Some(uninstall_command)) => {
                    fallback
                        .get_or_insert(NativeExeInstallMetadata::StandardOnly(uninstall_command));
                }
                (None, None) => continue,
            }
        }
    }

    fallback
}

fn same_install_location(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => normalize_path_text(left) == normalize_path_text(right),
    }
}

fn normalize_path_text(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
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

#[cfg(windows)]
fn build_install_args(
    installer: &CatalogInstaller,
    install_dir: &Path,
    package_name: &str,
) -> Result<Vec<String>> {
    let mut args = installer
        .installer_switches
        .as_deref()
        .map(split_switches)
        .transpose()?
        .unwrap_or_default();

    match installer.kind {
        InstallerType::Exe => {
            if args.is_empty() {
                bail!(
                    "missing installer switches for generic exe installer '{}'",
                    package_name
                );
            }
        }
        InstallerType::Inno => {
            push_flag_if_missing(&mut args, "/VERYSILENT");
            push_flag_if_missing(&mut args, "/SUPPRESSMSGBOXES");
            push_flag_if_missing(&mut args, "/NORESTART");
            push_flag_if_missing(&mut args, "/SP-");

            if !has_arg_prefix(&args, "/dir=") {
                args.push(format!(r"/DIR={}", install_dir.display()));
            }
        }
        InstallerType::Nullsoft => {
            push_flag_if_missing(&mut args, "/S");

            if !has_arg_prefix(&args, "/d=") {
                args.push(format!(r"/D={}", install_dir.display()));
            }
        }
        InstallerType::Burn => {
            push_flag_if_missing(&mut args, "/quiet");
            push_flag_if_missing(&mut args, "/norestart");
        }
        _ => {
            bail!(
                "native exe backend cannot handle installer kind '{}'",
                installer.kind.as_str()
            )
        }
    }

    Ok(args)
}

fn split_switches(raw: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in raw.chars() {
        match ch {
            '"' | '\'' => match quote {
                Some(active) if active == ch => {
                    quote = None;
                }
                Some(_) => current.push(ch),
                None => quote = Some(ch),
            },
            ch if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }

    if quote.is_some() {
        bail!("unterminated quoted installer switches: {raw}");
    }

    if !current.is_empty() {
        args.push(current);
    }

    Ok(args)
}

fn push_flag_if_missing(args: &mut Vec<String>, flag: &str) {
    if !args.iter().any(|arg| arg.eq_ignore_ascii_case(flag)) {
        args.push(flag.to_string());
    }
}

fn has_arg_prefix(args: &[String], prefix: &str) -> bool {
    args.iter()
        .any(|arg| arg.to_ascii_lowercase().starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::{has_arg_prefix, split_switches};

    #[cfg(windows)]
    use super::{NativeExeInstallMetadata, build_install_args, capture_native_exe_metadata};

    use std::path::PathBuf;

    use winbrew_models::catalog::package::CatalogInstaller;
    use winbrew_models::install::installer::InstallerType;
    use winbrew_models::shared::CatalogId;
    use winbrew_windows::create_test_uninstall_entry;

    #[cfg(windows)]
    fn native_exe_installer(kind: InstallerType, switches: Option<&str>) -> CatalogInstaller {
        let mut installer = CatalogInstaller::test_builder(
            CatalogId::parse("winget/Contoso.NativeExe").expect("catalog id should parse"),
            "https://example.invalid/setup.exe",
        )
        .with_kind(kind);

        if let Some(switches) = switches {
            installer = installer.with_installer_switches(switches);
        }

        installer
    }

    #[cfg(windows)]
    fn native_exe_test_dir(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("winbrew-nativeexe-{}-{suffix}", std::process::id()))
    }

    #[test]
    fn split_switches_preserves_quoted_arguments() {
        let args = split_switches(r#"/S /D="C:\Program Files\Demo" /quiet"#)
            .expect("switches should parse");

        assert_eq!(
            args,
            vec![
                "/S".to_string(),
                "/D=C:\\Program Files\\Demo".to_string(),
                "/quiet".to_string(),
            ]
        );
    }

    #[test]
    fn has_arg_prefix_detects_case_insensitive_prefixes() {
        let args = vec!["/DIR=C:\\Tools\\App".to_string()];

        assert!(has_arg_prefix(&args, "/dir="));
    }

    #[cfg(windows)]
    #[test]
    fn build_install_args_rejects_generic_exe_without_switches() {
        let installer = native_exe_installer(InstallerType::Exe, None);
        let install_dir = native_exe_test_dir("generic-exe");

        let err = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
            .expect_err("generic exe installs should require explicit switches");

        assert!(
            err.to_string().contains(
                "missing installer switches for generic exe installer 'Contoso.NativeExe'"
            )
        );
    }

    #[cfg(windows)]
    #[test]
    fn build_install_args_adds_inno_defaults_and_install_dir() {
        let installer = native_exe_installer(InstallerType::Inno, Some("/VERYSILENT"));
        let install_dir = native_exe_test_dir("inno");

        let args = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
            .expect("inno installs should build args");

        assert_eq!(
            args,
            vec![
                "/VERYSILENT".to_string(),
                "/SUPPRESSMSGBOXES".to_string(),
                "/NORESTART".to_string(),
                "/SP-".to_string(),
                format!(r"/DIR={}", install_dir.display()),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn capture_native_exe_metadata_reads_quiet_and_standard_commands() {
        let package_name = "Contoso.NativeExe";
        let install_dir = native_exe_test_dir("quiet-and-standard");
        std::fs::create_dir_all(&install_dir).expect("install directory should exist");
        let uninstall_exe = install_dir.join("uninstall.exe");
        let quiet_command = format!(r"{} /S", uninstall_exe.display());
        let expected_standard_command = uninstall_exe.display().to_string();
        let registry_entry = create_test_uninstall_entry(
            package_name,
            &install_dir,
            Some(quiet_command.as_str()),
            Some(expected_standard_command.as_str()),
        )
        .expect("test uninstall entry should be creatable");

        let metadata = capture_native_exe_metadata(package_name, &install_dir)
            .expect("metadata should be captured");

        assert!(matches!(
            metadata,
            NativeExeInstallMetadata::QuietAndStandard {
                ref quiet_uninstall_command,
                ref uninstall_command,
                ..
            } if quiet_uninstall_command == &quiet_command
                && uninstall_command == &expected_standard_command
        ));

        drop(registry_entry);
        let _ = std::fs::remove_dir_all(&install_dir);
    }

    #[cfg(windows)]
    #[test]
    fn capture_native_exe_metadata_falls_back_to_standard_command() {
        let package_name = "Contoso.NativeExe.Fallback";
        let install_dir = native_exe_test_dir("standard-only");
        std::fs::create_dir_all(&install_dir).expect("install directory should exist");
        let uninstall_exe = install_dir.join("uninstall.exe");
        let expected_uninstall_command = uninstall_exe.display().to_string();
        let registry_entry = create_test_uninstall_entry(
            package_name,
            &install_dir,
            None,
            Some(expected_uninstall_command.as_str()),
        )
        .expect("test uninstall entry should be creatable");

        let metadata = capture_native_exe_metadata(package_name, &install_dir)
            .expect("metadata should be captured");

        assert!(matches!(
            metadata,
            NativeExeInstallMetadata::StandardOnly(ref uninstall_command)
                if uninstall_command == &expected_uninstall_command
        ));

        drop(registry_entry);
        let _ = std::fs::remove_dir_all(&install_dir);
    }
}
