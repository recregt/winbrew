#![cfg(windows)]

mod common;

use std::fs;
use std::io::Write;

use common::{BASE_URL, assert_expected, installer, installer_with_url};
use tempfile::tempdir;
use winbrew_engines::{
    DeploymentKind, EngineKind, engine_kind_for_type, resolve_deployment_kind,
    resolve_downloaded_installer_kind, resolve_engine_for_installer,
};
use winbrew_models::install::installer::InstallerType;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[derive(Clone, Copy)]
struct EngineMappingCase {
    input: InstallerType,
    expected: EngineKind,
    description: &'static str,
}

#[derive(Clone, Copy)]
struct DeploymentScenario {
    kind: InstallerType,
    file_name: &'static str,
    nested_kind: Option<InstallerType>,
    expected: DeploymentKind,
    description: &'static str,
}

#[derive(Clone, Copy)]
struct RoutingScenario {
    kind: InstallerType,
    file_name: &'static str,
    nested_kind: Option<InstallerType>,
    expected: EngineKind,
    description: &'static str,
}

const ENGINE_MAPPING_CASES: &[EngineMappingCase] = &[
    EngineMappingCase {
        input: InstallerType::Msi,
        expected: EngineKind::Msi,
        description: "MSI maps to the MSI engine",
    },
    EngineMappingCase {
        input: InstallerType::Appx,
        expected: EngineKind::Msix,
        description: "Appx is treated as MSIX",
    },
    EngineMappingCase {
        input: InstallerType::Msix,
        expected: EngineKind::Msix,
        description: "MSIX stays MSIX",
    },
    EngineMappingCase {
        input: InstallerType::Zip,
        expected: EngineKind::Zip,
        description: "Zip maps to the archive engine",
    },
    EngineMappingCase {
        input: InstallerType::Portable,
        expected: EngineKind::Portable,
        description: "Portable stays portable",
    },
    EngineMappingCase {
        input: InstallerType::Exe,
        expected: EngineKind::NativeExe,
        description: "Generic EXE maps to the native executable engine",
    },
    EngineMappingCase {
        input: InstallerType::Font,
        expected: EngineKind::Font,
        description: "Font installers map to the font engine",
    },
];

const DEPLOYMENT_SCENARIOS: &[DeploymentScenario] = &[
    DeploymentScenario {
        kind: InstallerType::Portable,
        file_name: "tool.exe",
        nested_kind: None,
        expected: DeploymentKind::Portable,
        description: "Portable payloads without archive markers stay portable",
    },
    DeploymentScenario {
        kind: InstallerType::Zip,
        file_name: "tool.zip",
        nested_kind: None,
        expected: DeploymentKind::Portable,
        description: "Zip installers without nested kind still deploy as portable",
    },
    DeploymentScenario {
        kind: InstallerType::Zip,
        file_name: "tool.tar.gz",
        nested_kind: Some(InstallerType::Msi),
        expected: DeploymentKind::Installed,
        description: "Archive payloads can force installed deployment when nested MSI is present",
    },
    DeploymentScenario {
        kind: InstallerType::Msix,
        file_name: "package.msix",
        nested_kind: None,
        expected: DeploymentKind::Installed,
        description: "MSIX installers are installed deployments",
    },
];

const ROUTING_SCENARIOS: &[RoutingScenario] = &[
    RoutingScenario {
        kind: InstallerType::Msix,
        file_name: "package.msix",
        nested_kind: None,
        expected: EngineKind::Msix,
        description: "MSIX installers route to the MSIX engine",
    },
    RoutingScenario {
        kind: InstallerType::Portable,
        file_name: "tool.exe",
        nested_kind: None,
        expected: EngineKind::Portable,
        description: "Raw portable payloads stay on the portable engine",
    },
    RoutingScenario {
        kind: InstallerType::Portable,
        file_name: "tool.zip",
        nested_kind: None,
        expected: EngineKind::Zip,
        description: "Portable ZIP payloads route to the zip engine",
    },
    RoutingScenario {
        kind: InstallerType::Zip,
        file_name: "tool.tar.gz",
        nested_kind: Some(InstallerType::Msi),
        expected: EngineKind::Zip,
        description: "Archive kinds still resolve to the zip engine even with nested MSI",
    },
    RoutingScenario {
        kind: InstallerType::Exe,
        file_name: "setup.exe",
        nested_kind: None,
        expected: EngineKind::NativeExe,
        description: "Native EXE installers route to the native executable engine",
    },
];

#[derive(Clone, Copy)]
struct DownloadedRoutingScenario {
    kind: InstallerType,
    payload: &'static [u8],
    expected_kind: InstallerType,
    expected_engine: EngineKind,
    expected_deployment: DeploymentKind,
    description: &'static str,
}

const DOWNLOADED_ROUTING_SCENARIOS: &[DownloadedRoutingScenario] = &[
    DownloadedRoutingScenario {
        kind: InstallerType::Portable,
        payload: &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
        expected_kind: InstallerType::Msi,
        expected_engine: EngineKind::Msi,
        expected_deployment: DeploymentKind::Installed,
        description: "portable-labeled MSI payloads should route to the MSI engine",
    },
    DownloadedRoutingScenario {
        kind: InstallerType::Portable,
        payload: b"PK\x03\x04rest",
        expected_kind: InstallerType::Zip,
        expected_engine: EngineKind::Zip,
        expected_deployment: DeploymentKind::Portable,
        description: "portable-labeled archive payloads should route to the zip engine",
    },
    DownloadedRoutingScenario {
        kind: InstallerType::Msix,
        payload: b"PK\x03\x04rest",
        expected_kind: InstallerType::Msix,
        expected_engine: EngineKind::Msix,
        expected_deployment: DeploymentKind::Installed,
        description: "MSIX manifests should keep the MSIX engine even when the bytes look zip-like",
    },
    DownloadedRoutingScenario {
        kind: InstallerType::Portable,
        payload: b"plain text payload",
        expected_kind: InstallerType::Portable,
        expected_engine: EngineKind::Portable,
        expected_deployment: DeploymentKind::Portable,
        description: "unknown payloads should fall back to the manifest kind",
    },
];

#[test]
fn engine_kind_for_type_maps_supported_cases() {
    for case in ENGINE_MAPPING_CASES {
        assert_expected(
            engine_kind_for_type(case.input).unwrap(),
            case.expected,
            case.description,
        );
    }
}

#[test]
fn resolve_deployment_kind_uses_nested_installer_type_for_archives() {
    for case in DEPLOYMENT_SCENARIOS {
        let installer = installer(case.kind, case.file_name, case.nested_kind);

        assert_expected(
            resolve_deployment_kind(&installer),
            case.expected,
            case.description,
        );
    }
}

#[test]
fn resolve_engine_for_installer_routes_public_families() {
    for case in ROUTING_SCENARIOS {
        let installer = installer(case.kind, case.file_name, case.nested_kind);

        assert_expected(
            resolve_engine_for_installer(&installer).unwrap(),
            case.expected,
            case.description,
        );
    }
}

#[test]
fn resolve_downloaded_installer_kind_uses_probe_results() {
    for case in DOWNLOADED_ROUTING_SCENARIOS {
        let temp_dir = tempdir().expect("temp dir");
        let download_path = temp_dir.path().join("payload.bin");
        fs::write(&download_path, case.payload).expect("write payload");

        let installer = installer(case.kind, "payload.exe", None);
        let resolved_kind = resolve_downloaded_installer_kind(&installer, &download_path)
            .expect("resolve downloaded kind");

        assert_expected(resolved_kind, case.expected_kind, case.description);

        let mut resolved_installer = installer.clone();
        resolved_installer.kind = resolved_kind;

        assert_expected(
            resolve_engine_for_installer(&resolved_installer).unwrap(),
            case.expected_engine,
            case.description,
        );
        assert_expected(
            resolve_deployment_kind(&resolved_installer),
            case.expected_deployment,
            case.description,
        );
    }
}

#[test]
fn resolve_downloaded_installer_kind_detects_msix_like_zip_payloads() {
    let temp_dir = tempdir().expect("temp dir");
    let download_path = temp_dir.path().join("payload.zip");
    let file = fs::File::create(&download_path).expect("create zip file");
    let mut writer = ZipWriter::new(file);

    writer
        .start_file("AppxManifest.xml", SimpleFileOptions::default())
        .expect("start manifest entry");
    writer
        .write_all(b"<Package />")
        .expect("write manifest contents");
    writer.finish().expect("finish zip file");

    let installer = installer(InstallerType::Portable, "payload.zip", None);
    let resolved_kind = resolve_downloaded_installer_kind(&installer, &download_path)
        .expect("resolve downloaded kind");

    assert_expected(
        resolved_kind,
        InstallerType::Msix,
        "msix-shaped zip payloads should route to the msix family",
    );

    let mut resolved_installer = installer.clone();
    resolved_installer.kind = resolved_kind;

    assert_expected(
        resolve_engine_for_installer(&resolved_installer).unwrap(),
        EngineKind::Msix,
        "msix-shaped zip payloads should route to the msix engine",
    );
    assert_expected(
        resolve_deployment_kind(&resolved_installer),
        DeploymentKind::Installed,
        "msix-shaped zip payloads should deploy as installed packages",
    );
}

mod edge_cases {
    use super::*;

    #[test]
    fn resolve_deployment_kind_keeps_portable_payloads_portable_even_with_nested_metadata() {
        let installer = installer(
            InstallerType::Portable,
            "tool.tar.gz",
            Some(InstallerType::Msi),
        );

        assert_expected(
            resolve_deployment_kind(&installer),
            DeploymentKind::Portable,
            "portable installers should ignore nested metadata for deployment kind",
        );
    }

    #[test]
    fn resolve_engine_for_installer_handles_unicode_paths() {
        let installer = installer_with_url(
            InstallerType::Portable,
            "https://example.invalid/工具-portable.exe",
            None,
        );

        assert_expected(
            resolve_engine_for_installer(&installer).unwrap(),
            EngineKind::Portable,
            "unicode filenames should not break portable routing",
        );
    }

    #[test]
    fn resolve_engine_for_installer_handles_base_urls_without_file_names() {
        let installer = installer_with_url(InstallerType::Portable, BASE_URL, None);

        assert_expected(
            resolve_engine_for_installer(&installer).unwrap(),
            EngineKind::Portable,
            "base urls without a file name should fall back to portable",
        );
    }
}
