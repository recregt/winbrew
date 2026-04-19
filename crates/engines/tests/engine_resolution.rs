#![cfg(windows)]

mod common;

use common::installer;
use winbrew_engines::{
    DeploymentKind, EngineKind, engine_kind_for_type, resolve_deployment_kind,
    resolve_engine_for_installer,
};
use winbrew_models::install::installer::InstallerType;

#[test]
fn engine_kind_for_type_maps_supported_cases() {
    for (input, expected) in [
        (InstallerType::Msi, EngineKind::Msi),
        (InstallerType::Appx, EngineKind::Msix),
        (InstallerType::Msix, EngineKind::Msix),
        (InstallerType::Zip, EngineKind::Zip),
        (InstallerType::Portable, EngineKind::Portable),
        (InstallerType::Exe, EngineKind::NativeExe),
        (InstallerType::Font, EngineKind::Font),
    ] {
        assert_eq!(engine_kind_for_type(input).unwrap(), expected);
    }
}

#[test]
fn resolve_deployment_kind_uses_nested_installer_type_for_archives() {
    for (kind, file_name, nested_kind, expected) in [
        (
            InstallerType::Portable,
            "tool.exe",
            None,
            DeploymentKind::Portable,
        ),
        (
            InstallerType::Zip,
            "tool.zip",
            None,
            DeploymentKind::Portable,
        ),
        (
            InstallerType::Zip,
            "tool.tar.gz",
            Some(InstallerType::Msi),
            DeploymentKind::Installed,
        ),
        (
            InstallerType::Msix,
            "package.msix",
            None,
            DeploymentKind::Installed,
        ),
    ] {
        let installer = installer(kind, file_name, nested_kind);

        assert_eq!(resolve_deployment_kind(&installer), expected);
    }
}

#[test]
fn resolve_engine_for_installer_routes_public_families() {
    for (kind, file_name, nested_kind, expected) in [
        (InstallerType::Msix, "package.msix", None, EngineKind::Msix),
        (
            InstallerType::Portable,
            "tool.exe",
            None,
            EngineKind::Portable,
        ),
        (InstallerType::Portable, "tool.zip", None, EngineKind::Zip),
        (
            InstallerType::Zip,
            "tool.tar.gz",
            Some(InstallerType::Msi),
            EngineKind::Zip,
        ),
        (InstallerType::Exe, "setup.exe", None, EngineKind::NativeExe),
    ] {
        let installer = installer(kind, file_name, nested_kind);

        assert_eq!(resolve_engine_for_installer(&installer).unwrap(), expected);
    }
}
