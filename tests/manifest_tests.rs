use winbrew::manifest::{Manifest, Source};

#[test]
fn parse_toml_manifest_round_trips_basic_package_fields() {
    let content = r#"
        [manifest]
        manifest_type = "installer"
        manifest_version = "1.9.0"

        [package]
        name = "Microsoft.WindowsTerminal"
        version = "1.21.2361.0"
        package_name = "Windows Terminal"
        description = "Open source terminal application for developers."

        [source]
        url = "https://example.invalid/app.exe"
        checksum = "abc123"
        kind = "portable"
    "#;

    let manifest = Manifest::parse_toml(content).expect("manifest should parse");

    assert_eq!(manifest.manifest.manifest_type, "installer");
    assert_eq!(manifest.manifest.manifest_version, "1.9.0");
    assert_eq!(manifest.package.name, "Microsoft.WindowsTerminal");
    assert_eq!(
        manifest.package.package_name.as_deref(),
        Some("Windows Terminal")
    );
    assert_eq!(
        manifest.source.as_ref().map(|source| source.kind.as_str()),
        Some("portable")
    );
}

#[test]
fn parse_toml_manifest_rejects_invalid_input() {
    let error = Manifest::parse_toml("not valid toml").expect_err("invalid toml should fail");

    assert!(error.to_string().contains("failed to parse manifest"));
}

#[test]
fn source_validate_download_kind_accepts_expected_kinds() {
    for kind in ["portable", "msi", "msix"] {
        let source = Source {
            url: "https://example.invalid/app.exe".to_string(),
            checksum: "abc123".to_string(),
            kind: kind.to_string(),
        };

        assert!(source.validate_download_kind().is_ok());
    }
}

#[test]
fn source_validate_download_kind_rejects_unknown_kinds() {
    let source = Source {
        url: "https://example.invalid/app.zip".to_string(),
        checksum: "abc123".to_string(),
        kind: "zip".to_string(),
    };

    let error = source
        .validate_download_kind()
        .expect_err("unknown kind should fail");

    assert!(error.to_string().contains("unsupported download type"));
}
