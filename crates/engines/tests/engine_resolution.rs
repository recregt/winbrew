#![cfg(windows)]

mod common;

use common::{BASE_URL, assert_expected, installer, installer_with_url};
use winbrew_engines::{
    DeploymentKind, EngineKind, engine_kind_for_type, resolve_deployment_kind,
    resolve_engine_for_installer,
};
use winbrew_models::install::installer::InstallerType;

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
