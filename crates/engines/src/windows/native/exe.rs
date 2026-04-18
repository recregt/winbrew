//! Native executable installation and removal for Windows.
//!
//! This backend handles installer families that are executed as processes
//! rather than unpacked as files:
//!
//! - generic native `.exe` installers when explicit switches are provided
//! - Inno Setup installers
//! - Nullsoft / NSIS installers
//! - Burn bootstrapper installers
//!
//! What this module does:
//!
//! - validates the installer path, install directory, and package name before
//!   starting work
//! - parses installer switches literally and rejects duplicate installer
//!   switches before
//!   execution, so catalog mistakes fail fast instead of being silently
//!   normalized
//! - launches the downloaded installer as a process and treats the Windows
//!   installer success codes `0`, `1641`, and `3010` as successful outcomes
//! - captures uninstall metadata from the Windows uninstall registry when it
//!   can, so later removal can reuse the recorded command
//! - falls back to direct directory cleanup when uninstall metadata is missing
//!   or the uninstall command fails
//!
//! What this module does not do:
//!
//! - it does not extract archives or copy payload files
//! - it does not infer installer family from URLs alone
//! - it does not own MSIX / App Installer behavior, which lives in the MSIX
//!   API adapter

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
use winbrew_windows::collect_uninstall_entries;

const NATIVE_EXE_SUCCESS_EXIT_CODES: &[i32] = &[0, 1641, 3010];

/// Install a native executable package by running the downloaded installer.
///
/// The installer family is expected to come from catalog metadata. The backend
/// validates the inputs, builds family-specific switches, executes the installer
/// process, and records uninstall metadata when Windows exposes it.
pub fn install(
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

fn validate_install_inputs(
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<()> {
    validate_download_path(download_path)?;
    validate_install_dir(install_dir)?;
    validate_package_name(package_name)?;

    Ok(())
}

fn validate_download_path(path: &Path) -> Result<()> {
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

fn validate_install_dir(path: &Path) -> Result<()> {
    let path_text = path.to_string_lossy();

    if path.as_os_str().is_empty() || path_text.trim().is_empty() {
        bail!("install directory cannot be empty");
    }

    Ok(())
}

fn validate_package_name(package_name: &str) -> Result<()> {
    let package_name = package_name.trim();

    if package_name.is_empty() {
        bail!("package name cannot be empty");
    }

    if package_name.chars().any(|ch| ch.is_control()) {
        bail!("package name contains invalid control characters");
    }

    Ok(())
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
    let package_name = package_name.trim();
    let mut best_match: Option<(u8, NativeExeInstallMetadata)> = None;
    let mut saw_ambiguous_match = false;

    let Ok(entries) = collect_uninstall_entries(Some(package_name)) else {
        return None;
    };

    for entry in entries {
        if !entry.display_name.trim().eq_ignore_ascii_case(package_name) {
            continue;
        }

        let install_location_exact = match entry.install_location.as_deref() {
            Some(value) if !value.trim().is_empty() => {
                if !same_install_location(Path::new(value), install_dir) {
                    continue;
                }

                true
            }
            _ => false,
        };

        let candidate = match (
            entry.quiet_uninstall_string.as_deref(),
            entry.uninstall_string.as_deref(),
        ) {
            (Some(quiet_uninstall_command), Some(uninstall_command)) => Some((
                native_exe_metadata_priority(install_location_exact, 3),
                NativeExeInstallMetadata::QuietAndStandard {
                    quiet_uninstall_command: quiet_uninstall_command.to_string(),
                    uninstall_command: uninstall_command.to_string(),
                },
            )),
            (Some(quiet_uninstall_command), None) => Some((
                native_exe_metadata_priority(install_location_exact, 2),
                NativeExeInstallMetadata::QuietOnly(quiet_uninstall_command.to_string()),
            )),
            (None, Some(uninstall_command)) => Some((
                native_exe_metadata_priority(install_location_exact, 1),
                NativeExeInstallMetadata::StandardOnly(uninstall_command.to_string()),
            )),
            (None, None) => None,
        };

        let Some((priority, metadata)) = candidate else {
            continue;
        };

        match best_match.as_mut() {
            Some((best_priority, best_metadata)) => {
                if priority > *best_priority {
                    *best_priority = priority;
                    *best_metadata = metadata;
                } else if priority == *best_priority {
                    saw_ambiguous_match = true;
                }
            }
            None => {
                best_match = Some((priority, metadata));
            }
        }
    }

    if saw_ambiguous_match {
        warn!(
            package = package_name,
            install_dir = %install_dir.display(),
            "multiple native executable uninstall registry entries matched; using the best available metadata"
        );
    }

    best_match.map(|(_, metadata)| metadata)
}

fn native_exe_metadata_priority(install_location_exact: bool, metadata_priority: u8) -> u8 {
    if install_location_exact {
        10 + metadata_priority
    } else {
        metadata_priority
    }
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

    validate_unique_switches(&args, raw)?;

    Ok(args)
}

fn validate_unique_switches(args: &[String], raw: &str) -> Result<()> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();

    for arg in args {
        let signature = switch_signature(arg);

        if !seen.insert(signature) {
            bail!("duplicate installer switch detected: {arg} in {raw}");
        }
    }

    Ok(())
}

fn switch_signature(arg: &str) -> String {
    let trimmed = arg.trim();

    match trimmed.split_once('=') {
        Some((left, _)) => format!("{}=", left.to_ascii_lowercase()),
        None => trimmed.to_ascii_lowercase(),
    }
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
    use super::{
        has_arg_prefix, split_switches, validate_download_path, validate_install_dir,
        validate_package_name,
    };

    #[cfg(windows)]
    use super::{NativeExeInstallMetadata, build_install_args, capture_native_exe_metadata};

    use std::path::{Path, PathBuf};

    use winbrew_models::catalog::package::CatalogInstaller;
    use winbrew_models::install::installer::InstallerType;
    use winbrew_models::shared::CatalogId;
    use winbrew_windows::{
        create_test_uninstall_entry, create_test_uninstall_entry_with_install_location,
    };

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
    fn split_switches_rejects_unterminated_quotes() {
        let err = split_switches(r#"/S /D="C:\Program Files\Demo"#)
            .expect_err("unterminated quotes should fail");

        assert!(
            err.to_string()
                .contains("unterminated quoted installer switches")
        );
    }

    #[test]
    fn split_switches_rejects_duplicate_flags() {
        let err = split_switches(r#"/S /quiet /s"#).expect_err("duplicate flags should fail");

        assert!(
            err.to_string()
                .contains("duplicate installer switch detected")
        );
    }

    #[test]
    fn split_switches_rejects_duplicate_value_switches() {
        let err = split_switches(r#"ALLUSERS=1 ALLUSERS=0"#)
            .expect_err("duplicate value switches should fail");

        assert!(
            err.to_string()
                .contains("duplicate installer switch detected")
        );
    }

    #[test]
    fn has_arg_prefix_detects_case_insensitive_prefixes() {
        let args = vec!["/DIR=C:\\Tools\\App".to_string()];

        assert!(has_arg_prefix(&args, "/dir="));
    }

    #[test]
    fn validate_download_path_rejects_missing_file() {
        let path = native_exe_test_dir("missing-installer").join("installer.exe");

        let err = validate_download_path(&path).expect_err("missing installer should fail");

        assert!(err.to_string().contains("installer path does not exist"));
    }

    #[test]
    fn validate_install_dir_rejects_empty_path() {
        let err =
            validate_install_dir(Path::new("")).expect_err("empty install directory should fail");

        assert!(
            err.to_string()
                .contains("install directory cannot be empty")
        );
    }

    #[test]
    fn validate_package_name_rejects_empty_and_control_characters() {
        let empty_err = validate_package_name("   ").expect_err("empty package name should fail");
        assert!(
            empty_err
                .to_string()
                .contains("package name cannot be empty")
        );

        let control_err = validate_package_name("Contoso\nNativeExe")
            .expect_err("control characters should fail");
        assert!(
            control_err
                .to_string()
                .contains("package name contains invalid control characters")
        );
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
    fn build_install_args_adds_nullsoft_defaults_and_install_dir() {
        let installer = native_exe_installer(InstallerType::Nullsoft, Some("/S"));
        let install_dir = native_exe_test_dir("nullsoft");

        let args = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
            .expect("nullsoft installs should build args");

        assert_eq!(
            args,
            vec!["/S".to_string(), format!(r"/D={}", install_dir.display())]
        );
    }

    #[cfg(windows)]
    #[test]
    fn build_install_args_adds_burn_defaults() {
        let installer = native_exe_installer(InstallerType::Burn, Some("/quiet"));
        let install_dir = native_exe_test_dir("burn");

        let args = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
            .expect("burn installs should build args");

        assert_eq!(args, vec!["/quiet".to_string(), "/norestart".to_string()]);
    }

    #[cfg(windows)]
    #[test]
    fn build_install_args_preserves_generic_exe_switches() {
        let installer = native_exe_installer(
            InstallerType::Exe,
            Some(r#"/quiet /D="C:\Program Files\Demo""#),
        );
        let install_dir = native_exe_test_dir("generic-preserve");

        let args = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
            .expect("generic exe installs should preserve explicit switches");

        assert_eq!(
            args,
            vec![
                "/quiet".to_string(),
                "/D=C:\\Program Files\\Demo".to_string(),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn build_install_args_rejects_duplicate_switches() {
        let installer = native_exe_installer(InstallerType::Exe, Some(r#"/quiet /quiet"#));
        let install_dir = native_exe_test_dir("duplicate-switches");

        let err = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
            .expect_err("duplicate installer switches should fail");

        assert!(
            err.to_string()
                .contains("duplicate installer switch detected")
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

    #[cfg(windows)]
    #[test]
    fn capture_native_exe_metadata_prefers_exact_install_location_over_locationless_fallback() {
        let package_name = "Contoso.NativeExe.Preference";
        let exact_install_dir = native_exe_test_dir("exact-location");
        let fallback_install_dir = native_exe_test_dir("locationless-fallback");
        std::fs::create_dir_all(&exact_install_dir).expect("exact install directory should exist");
        std::fs::create_dir_all(&fallback_install_dir)
            .expect("fallback install directory should exist");

        let fallback_uninstall_exe = fallback_install_dir.join("uninstall.exe");
        let fallback_uninstall_command = fallback_uninstall_exe.display().to_string();
        let fallback_registry_entry = create_test_uninstall_entry_with_install_location(
            package_name,
            None,
            None,
            Some(fallback_uninstall_command.as_str()),
        )
        .expect("locationless fallback uninstall entry should be creatable");

        let exact_uninstall_exe = exact_install_dir.join("uninstall.exe");
        let exact_uninstall_command = exact_uninstall_exe.display().to_string();
        let exact_registry_entry = create_test_uninstall_entry(
            package_name,
            &exact_install_dir,
            None,
            Some(exact_uninstall_command.as_str()),
        )
        .expect("exact uninstall entry should be creatable");

        let metadata = capture_native_exe_metadata(package_name, &exact_install_dir)
            .expect("metadata should be captured");

        assert!(matches!(
            metadata,
            NativeExeInstallMetadata::StandardOnly(ref uninstall_command)
                if uninstall_command == &exact_uninstall_command
        ));

        drop(exact_registry_entry);
        drop(fallback_registry_entry);
        let _ = std::fs::remove_dir_all(&exact_install_dir);
        let _ = std::fs::remove_dir_all(&fallback_install_dir);
    }

    #[cfg(windows)]
    #[test]
    fn capture_native_exe_metadata_returns_none_when_registry_entry_missing() {
        let package_name = "Contoso.NativeExe.Missing";
        let install_dir = native_exe_test_dir("missing-metadata");
        std::fs::create_dir_all(&install_dir).expect("install directory should exist");

        let metadata = capture_native_exe_metadata(package_name, &install_dir);

        assert!(metadata.is_none());

        let _ = std::fs::remove_dir_all(&install_dir);
    }
}
