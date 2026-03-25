use winbrew::core::install::{build_plan, detect_ext, source_file_name};
use winbrew::manifest::{InstallerEntry, Manifest, ManifestInfo, Package, Source};

fn manifest_with_installer(
    installer_type: &str,
    url: &str,
    product_code: Option<&str>,
) -> Manifest {
    Manifest {
        manifest: ManifestInfo::default(),
        package: Package {
            name: "Microsoft.WindowsTerminal".to_string(),
            version: "1.21.2361.0".to_string(),
            package_name: Some("Windows Terminal".to_string()),
            description: Some("Terminal".to_string()),
            publisher: Some("Microsoft Corporation".to_string()),
            homepage: None,
            license: None,
            moniker: None,
            tags: vec![],
            dependencies: vec!["Microsoft.VCLibs".to_string()],
        },
        source: None,
        installers: vec![InstallerEntry {
            architecture: "x64".to_string(),
            installer_type: installer_type.to_string(),
            installer_url: url.to_string(),
            installer_sha256: "abc123".to_string(),
            installer_locale: None,
            scope: None,
            product_code: product_code.map(ToOwned::to_owned),
            release_date: None,
            display_name: None,
            upgrade_behavior: None,
        }],
        metadata: None,
    }
}

fn manifest_with_source(url: &str, kind: &str) -> Manifest {
    Manifest {
        manifest: ManifestInfo::default(),
        package: Package {
            name: "Microsoft.WindowsTerminal".to_string(),
            version: "1.21.2361.0".to_string(),
            package_name: Some("Windows Terminal".to_string()),
            description: Some("Terminal".to_string()),
            publisher: Some("Microsoft Corporation".to_string()),
            homepage: None,
            license: None,
            moniker: None,
            tags: vec![],
            dependencies: vec![],
        },
        source: Some(Source {
            url: url.to_string(),
            checksum: "abc123".to_string(),
            kind: kind.to_string(),
        }),
        installers: vec![],
        metadata: None,
    }
}

#[test]
fn detect_ext_ignores_query_string() {
    assert_eq!(
        detect_ext("https://example.invalid/app.msi?download=1"),
        "msi"
    );
}

#[test]
fn source_file_name_ignores_fragment_and_query() {
    assert_eq!(
        source_file_name("https://example.invalid/app.msixbundle?download=1#anchor"),
        Some("app.msixbundle".to_string())
    );
}

#[test]
fn build_plan_uses_preferred_installer_and_install_root() {
    let manifest = manifest_with_installer(
        "msi",
        "https://example.invalid/WindowsTerminal.msi",
        Some("{11111111-1111-1111-1111-111111111111}"),
    );

    let plan = build_plan("Microsoft.WindowsTerminal", &manifest).expect("plan should build");

    assert_eq!(plan.name, "Microsoft.WindowsTerminal");
    assert_eq!(plan.package_version, "1.21.2361.0");
    assert_eq!(plan.source.kind, "msi");
    assert_eq!(
        plan.product_code.as_deref(),
        Some("{11111111-1111-1111-1111-111111111111}")
    );
    assert!(
        plan.install_dir
            .ends_with(r"packages\Microsoft.WindowsTerminal")
    );
    assert_eq!(
        plan.cache_file,
        winbrew::core::paths::cache_file("Microsoft.WindowsTerminal", "1.21.2361.0", "msi")
    );
    assert_eq!(plan.backup_dir.parent(), plan.install_dir.parent());
    assert_eq!(
        plan.backup_dir.extension().and_then(|ext| ext.to_str()),
        Some("backup")
    );
    assert_eq!(plan.dependencies, vec!["Microsoft.VCLibs".to_string()]);
}

#[test]
fn build_plan_falls_back_to_manifest_source_when_installers_are_missing() {
    let manifest = manifest_with_source("https://example.invalid/WindowsTerminal.zip", "portable");

    let plan = build_plan("Microsoft.WindowsTerminal", &manifest).expect("plan should build");

    assert_eq!(plan.source.kind, "portable");
    assert_eq!(
        plan.source.url,
        "https://example.invalid/WindowsTerminal.zip"
    );
    assert!(plan.product_code.is_none());
}

#[test]
fn build_plan_rejects_traversal_package_names() {
    let manifest = manifest_with_installer(
        "msi",
        "https://example.invalid/WindowsTerminal.msi",
        Some("{11111111-1111-1111-1111-111111111111}"),
    );

    let err = build_plan("..", &manifest).expect_err("plan should fail");

    assert!(
        err.to_string()
            .contains("invalid package name: traversal characters detected")
    );
}

#[test]
fn build_plan_rejects_windows_forbidden_filename_characters() {
    let manifest = manifest_with_installer(
        "msi",
        "https://example.invalid/WindowsTerminal.msi",
        Some("{11111111-1111-1111-1111-111111111111}"),
    );

    let err = build_plan("Microsoft:WindowsTerminal", &manifest).expect_err("plan should fail");

    assert!(
        err.to_string()
            .contains("invalid package name: forbidden Windows filename characters detected")
    );
}
