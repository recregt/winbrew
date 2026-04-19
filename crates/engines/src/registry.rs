use anyhow::{Result, anyhow};
use std::path::Path;

use crate::models::catalog::package::CatalogInstaller;
use crate::models::install::engine::EngineInstallReceipt;
use crate::models::install::installed::InstalledPackage;
use crate::models::install::installer::InstallerType;
use crate::models::shared::DeploymentKind;

use super::EngineKind;
use crate::filesystem::{archive::zip, portable};
use crate::payload::{
    DetectedArtifactKind, PayloadKind, classify_payload, probe_downloaded_artifact_kind,
};
use crate::windows::api::msix;
use crate::windows::font;

#[cfg(windows)]
use crate::windows::native::{exe, msi};

type InstallFn = fn(&CatalogInstaller, &Path, &Path, &str) -> Result<EngineInstallReceipt>;
type RemoveFn = fn(&InstalledPackage) -> Result<()>;
type MatchesInstallerFn = fn(&CatalogInstaller) -> bool;

struct EngineDescriptor {
    kind: EngineKind,
    install: InstallFn,
    remove: RemoveFn,
    matches_installer: MatchesInstallerFn,
}

fn matches_msix_installer(installer: &CatalogInstaller) -> bool {
    installer.kind.is_windows_package()
}

fn matches_native_exe_installer(installer: &CatalogInstaller) -> bool {
    installer.kind.is_native_exe_family()
}

fn matches_font_installer(installer: &CatalogInstaller) -> bool {
    installer.kind.is_font_family()
}

#[cfg(windows)]
fn matches_msi_installer(installer: &CatalogInstaller) -> bool {
    installer.kind.is_msi_family()
}

fn matches_archive_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Zip
        || matches!(classify_payload(&installer.url), PayloadKind::Archive(_))
}

fn matches_portable_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Portable
        && matches!(classify_payload(&installer.url), PayloadKind::Raw)
}

pub(crate) fn resolve_downloaded_installer_kind(
    installer: &CatalogInstaller,
    download_path: &Path,
) -> Result<InstallerType> {
    if installer.kind.is_windows_package() {
        return Ok(installer.kind);
    }

    match probe_downloaded_artifact_kind(download_path)? {
        Some(DetectedArtifactKind::Msi) => Ok(InstallerType::Msi),
        Some(DetectedArtifactKind::Archive(_)) => Ok(InstallerType::Zip),
        None => Ok(installer.kind),
    }
}

pub(crate) fn resolve_deployment_kind(installer: &CatalogInstaller) -> DeploymentKind {
    if installer.kind.is_archive() {
        return installer
            .nested_kind
            .map_or(DeploymentKind::Portable, InstallerType::deployment_kind);
    }

    installer.kind.deployment_kind()
}

fn msix_install(
    _installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    msix::install(download_path, install_dir, package_name)
}

fn native_exe_install(
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
        exe::install(installer, download_path, install_dir, package_name)
    }
}

fn font_install(
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    #[cfg(not(windows))]
    {
        let _ = (installer, download_path, install_dir, package_name);
        bail!("font installation is only supported on Windows")
    }

    #[cfg(windows)]
    {
        font::install(installer, download_path, install_dir, package_name)
    }
}

#[cfg(windows)]
fn msi_install(
    _installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    msi::install(download_path, install_dir, package_name)
}

fn zip_install(
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    _package_name: &str,
) -> Result<EngineInstallReceipt> {
    zip::install::install(download_path, install_dir, &installer.url)
}

fn portable_install(
    _installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    portable::install::install(download_path, install_dir, package_name)
}

fn msix_remove(package: &InstalledPackage) -> Result<()> {
    msix::remove(package)
}

fn native_exe_remove(package: &InstalledPackage) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = package;
        bail!("native executable removal is only supported on Windows")
    }

    #[cfg(windows)]
    {
        exe::remove(package)
    }
}

fn font_remove(package: &InstalledPackage) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = package;
        bail!("font removal is only supported on Windows")
    }

    #[cfg(windows)]
    {
        font::remove(package)
    }
}

#[cfg(windows)]
fn msi_remove(package: &InstalledPackage) -> Result<()> {
    msi::remove(package)
}

fn zip_remove(package: &InstalledPackage) -> Result<()> {
    zip::remove::remove(package)
}

fn portable_remove(package: &InstalledPackage) -> Result<()> {
    portable::remove::remove(package)
}

// Native executable and font families must appear before Zip so explicit
// installer kinds win over archive URL heuristics. Zip must still appear before
// Portable so archive payloads do not fall back to the raw-copy engine.
const ENGINE_DESCRIPTORS: &[EngineDescriptor] = &[
    #[cfg(windows)]
    EngineDescriptor {
        kind: EngineKind::Msi,
        install: msi_install,
        remove: msi_remove,
        matches_installer: matches_msi_installer,
    },
    EngineDescriptor {
        kind: EngineKind::Msix,
        install: msix_install,
        remove: msix_remove,
        matches_installer: matches_msix_installer,
    },
    EngineDescriptor {
        kind: EngineKind::NativeExe,
        install: native_exe_install,
        remove: native_exe_remove,
        matches_installer: matches_native_exe_installer,
    },
    EngineDescriptor {
        kind: EngineKind::Font,
        install: font_install,
        remove: font_remove,
        matches_installer: matches_font_installer,
    },
    EngineDescriptor {
        kind: EngineKind::Zip,
        install: zip_install,
        remove: zip_remove,
        matches_installer: matches_archive_installer,
    },
    EngineDescriptor {
        kind: EngineKind::Portable,
        install: portable_install,
        remove: portable_remove,
        matches_installer: matches_portable_installer,
    },
];

pub(crate) fn resolve_engine_kind_for_installer(
    installer: &CatalogInstaller,
) -> Result<EngineKind> {
    ENGINE_DESCRIPTORS
        .iter()
        .find(|descriptor| (descriptor.matches_installer)(installer))
        .map(|descriptor| descriptor.kind)
        .ok_or_else(|| anyhow!("unsupported installer type '{}'", installer.kind.as_str()))
}

pub(crate) fn install(
    kind: EngineKind,
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    let descriptor = resolve_engine_descriptor(kind)?;

    (descriptor.install)(installer, download_path, install_dir, package_name)
}

pub(crate) fn remove(kind: EngineKind, package: &InstalledPackage) -> Result<()> {
    let descriptor = resolve_engine_descriptor(kind)?;

    (descriptor.remove)(package)
}

fn resolve_engine_descriptor(kind: EngineKind) -> Result<&'static EngineDescriptor> {
    ENGINE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.kind == kind)
        .ok_or_else(|| anyhow!("unsupported engine kind: {:?}", kind))
}

#[cfg(test)]
mod tests {
    use super::{resolve_deployment_kind, resolve_engine_kind_for_installer};
    use crate::EngineKind;
    use crate::models::catalog::package::CatalogInstaller;
    use crate::models::install::installer::InstallerType;
    use crate::models::shared::DeploymentKind;

    fn installer(kind: InstallerType, url: &str) -> CatalogInstaller {
        CatalogInstaller::test_builder("Contoso.App".into(), url).with_kind(kind)
    }

    #[test]
    fn resolve_installer_treats_portable_zip_as_zip() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Portable,
            "https://example.invalid/tool.zip",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Zip);
    }

    #[test]
    fn resolve_installer_routes_raw_portable_payloads_to_portable() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Portable,
            "https://example.invalid/tool.exe",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Portable);
    }

    #[test]
    fn resolve_installer_routes_portable_archive_payloads_to_zip() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Portable,
            "https://example.invalid/tool.tar.gz",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Zip);
    }

    #[test]
    fn resolve_installer_routes_portable_gzip_payloads_to_zip() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Portable,
            "https://example.invalid/tool.gz",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Zip);
    }

    #[test]
    fn resolve_installer_prefers_msix_for_msix_kind() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Msix,
            "https://example.invalid/package.msix",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Msix);
    }

    #[test]
    fn resolve_installer_routes_native_exe_family_to_native_exe() {
        for kind in [
            InstallerType::Exe,
            InstallerType::Inno,
            InstallerType::Nullsoft,
            InstallerType::Burn,
        ] {
            let engine = resolve_engine_kind_for_installer(&installer(
                kind,
                "https://example.invalid/native-installer.exe",
            ))
            .expect("engine should resolve");

            assert_eq!(engine, EngineKind::NativeExe);
        }
    }

    #[test]
    fn resolve_installer_prefers_explicit_native_exe_kind_over_archive_url() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Exe,
            "https://example.invalid/native-installer.zip",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::NativeExe);
    }

    #[test]
    fn resolve_installer_keeps_pwa_unsupported() {
        let err = resolve_engine_kind_for_installer(&installer(
            InstallerType::Pwa,
            "https://example.invalid/special-installer.exe",
        ))
        .expect_err("pwa should not route yet");

        assert!(err.to_string().contains("unsupported installer type"));
    }

    #[test]
    fn resolve_installer_routes_font_to_font_engine() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Font,
            "https://example.invalid/font.ttf",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Font);
    }

    #[test]
    fn resolve_deployment_kind_uses_nested_installer_type_for_zip_archives() {
        let installer = installer(InstallerType::Zip, "https://example.invalid/package.zip")
            .with_nested(InstallerType::Msi);

        assert_eq!(
            resolve_deployment_kind(&installer),
            DeploymentKind::Installed
        );
    }

    #[test]
    fn resolve_deployment_kind_defaults_native_exe_family_to_installed() {
        for kind in [
            InstallerType::Exe,
            InstallerType::Inno,
            InstallerType::Nullsoft,
            InstallerType::Burn,
        ] {
            let installer = installer(kind, "https://example.invalid/native-installer.exe");

            assert_eq!(
                resolve_deployment_kind(&installer),
                DeploymentKind::Installed
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn resolve_installer_prefers_msi_for_msi_kind() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Msi,
            "https://example.invalid/package.msi",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Msi);
    }
}
