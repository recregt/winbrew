use anyhow::{Result, anyhow};
use std::path::Path;

use winbrew_models::catalog::package::CatalogInstaller;
use winbrew_models::install::engine::EngineInstallReceipt;
use winbrew_models::install::installed::InstalledPackage;
use winbrew_models::install::installer::InstallerType;

use super::EngineKind;
use crate::filesystem::{archive::zip, portable};
use crate::payload::{PayloadKind, classify_payload};
use crate::windows::package::msix;

#[cfg(windows)]
use crate::windows::native::msi;

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
    installer.kind == InstallerType::Msix
}

#[cfg(windows)]
fn matches_msi_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Msi
}

fn matches_archive_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Zip
        || matches!(classify_payload(&installer.url), PayloadKind::Archive(_))
}

fn matches_portable_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Portable
        && matches!(classify_payload(&installer.url), PayloadKind::Raw)
}

fn msix_install(
    _installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    msix::install::install(download_path, install_dir, package_name)
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
    msix::remove::remove(package)
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

// Archive payloads must appear before Portable so archive installers route to the
// archive engine while Portable remains the raw-copy fallback.
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
    use super::resolve_engine_kind_for_installer;
    use crate::EngineKind;
    use winbrew_models::catalog::package::CatalogInstaller;
    use winbrew_models::install::installer::InstallerType;

    fn installer(kind: InstallerType, url: &str) -> CatalogInstaller {
        CatalogInstaller {
            package_id: "Contoso.App".into(),
            url: url.to_string(),
            hash: "hash".to_string(),
            arch: "x64".parse().expect("arch should parse"),
            kind,
        }
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
    fn resolve_installer_prefers_msix_for_msix_kind() {
        let engine = resolve_engine_kind_for_installer(&installer(
            InstallerType::Msix,
            "https://example.invalid/package.msix",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Msix);
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
