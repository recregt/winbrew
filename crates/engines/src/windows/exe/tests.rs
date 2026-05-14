use super::metadata::{
    NativeExeInstallMetadata, capture_native_exe_metadata, capture_native_exe_metadata_with,
};
use super::switches::{build_install_args, has_arg_prefix, split_switches};
use super::validation::{validate_download_path, validate_install_dir, validate_package_name};

use std::path::{Path, PathBuf};

use crate::models::catalog::package::CatalogInstaller;
use crate::models::install::installer::InstallerType;
use crate::models::shared::CatalogId;
use crate::windows_dep::testing::create_test_uninstall_entry;
use winbrew_testing::{CatalogInstallerBuilderExt as _, catalog_installer};

fn native_exe_installer(kind: InstallerType, switches: Option<&str>) -> CatalogInstaller {
    let mut installer = catalog_installer(
        CatalogId::parse("winget/Contoso.NativeExe").expect("catalog id should parse"),
        "https://example.invalid/setup.exe",
    )
    .with_kind(kind);

    if let Some(switches) = switches {
        installer = installer.with_installer_switches(switches);
    }

    installer
}

fn native_exe_test_dir(suffix: &str) -> PathBuf {
    tempfile::Builder::new()
        .prefix(&format!(
            "winbrew-nativeexe-{}-{suffix}-",
            std::process::id()
        ))
        .tempdir()
        .expect("create native exe test dir")
        .keep()
}

fn uninstall_entry(
    package_name: &str,
    install_location: Option<&Path>,
    quiet_uninstall_string: Option<&str>,
    uninstall_string: Option<&str>,
) -> crate::windows_dep::installed::UninstallEntry {
    crate::windows_dep::installed::UninstallEntry {
        display_name: package_name.to_string(),
        version: String::new(),
        publisher: String::new(),
        install_location: install_location.map(|path| path.to_string_lossy().into_owned()),
        quiet_uninstall_string: quiet_uninstall_string.map(str::to_string),
        uninstall_string: uninstall_string.map(str::to_string),
    }
}

#[test]
fn split_switches_preserves_quoted_arguments() {
    let args =
        split_switches(r#"/S /D="C:\Program Files\Demo" /quiet"#).expect("switches should parse");

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
    let err = validate_install_dir(Path::new("")).expect_err("empty install directory should fail");

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

    let control_err =
        validate_package_name("Contoso\nNativeExe").expect_err("control characters should fail");
    assert!(
        control_err
            .to_string()
            .contains("package name contains invalid control characters")
    );
}

#[test]
fn build_install_args_rejects_generic_exe_without_switches() {
    let installer = native_exe_installer(InstallerType::Exe, None);
    let install_dir = native_exe_test_dir("generic-exe");

    let err = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
        .expect_err("generic exe installs should require explicit switches");

    assert!(
        err.to_string()
            .contains("missing installer switches for generic exe installer 'Contoso.NativeExe'")
    );
}

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

#[test]
fn build_install_args_adds_burn_defaults() {
    let installer = native_exe_installer(InstallerType::Burn, Some("/quiet"));
    let install_dir = native_exe_test_dir("burn");

    let args = build_install_args(&installer, &install_dir, "Contoso.NativeExe")
        .expect("burn installs should build args");

    assert_eq!(args, vec!["/quiet".to_string(), "/norestart".to_string()]);
}

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

#[test]
fn capture_native_exe_metadata_reads_quiet_only_commands() {
    let package_name = "Contoso.NativeExe.QuietOnly";
    let install_dir = native_exe_test_dir("quiet-only");
    std::fs::create_dir_all(&install_dir).expect("install directory should exist");
    let uninstall_exe = install_dir.join("uninstall.exe");
    let quiet_uninstall_command = format!(r"{} /S", uninstall_exe.display());
    let registry_entry = create_test_uninstall_entry(
        package_name,
        &install_dir,
        Some(quiet_uninstall_command.as_str()),
        None,
    )
    .expect("test uninstall entry should be creatable");

    let metadata = capture_native_exe_metadata(package_name, &install_dir)
        .expect("metadata should be captured");

    assert!(matches!(
        metadata,
        NativeExeInstallMetadata::QuietOnly(ref uninstall_command)
            if uninstall_command == &quiet_uninstall_command
    ));

    drop(registry_entry);
    let _ = std::fs::remove_dir_all(&install_dir);
}

#[test]
fn capture_native_exe_metadata_returns_none_when_registry_lookup_fails() {
    let package_name = "Contoso.NativeExe.RegistryFailure";
    let install_dir = PathBuf::from(r"C:\Contoso\NativeExe");

    let metadata = capture_native_exe_metadata_with(package_name, &install_dir, |_filter| {
        Err(anyhow::anyhow!("registry unavailable"))
    });

    assert!(metadata.is_none());
}

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

    let exact_uninstall_exe = exact_install_dir.join("uninstall.exe");
    let exact_uninstall_command = exact_uninstall_exe.display().to_string();

    let metadata = capture_native_exe_metadata_with(package_name, &exact_install_dir, |_filter| {
        Ok(vec![
            uninstall_entry(
                package_name,
                None,
                None,
                Some(fallback_uninstall_command.as_str()),
            ),
            uninstall_entry(
                package_name,
                Some(&exact_install_dir),
                None,
                Some(exact_uninstall_command.as_str()),
            ),
        ])
    })
    .expect("metadata should be captured");

    assert!(matches!(
        metadata,
        NativeExeInstallMetadata::StandardOnly(ref uninstall_command)
            if uninstall_command == &exact_uninstall_command
    ));

    let _ = std::fs::remove_dir_all(&exact_install_dir);
    let _ = std::fs::remove_dir_all(&fallback_install_dir);
}

#[test]
fn capture_native_exe_metadata_returns_none_when_registry_entry_missing() {
    let package_name = "Contoso.NativeExe.Missing";
    let install_dir = native_exe_test_dir("missing-metadata");
    std::fs::create_dir_all(&install_dir).expect("install directory should exist");

    let metadata = capture_native_exe_metadata(package_name, &install_dir);

    assert!(metadata.is_none());

    let _ = std::fs::remove_dir_all(&install_dir);
}
