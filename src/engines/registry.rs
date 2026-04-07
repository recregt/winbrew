use anyhow::{Result, anyhow};
use std::path::Path;

use crate::core::network::is_zip_path;
use crate::models::{CatalogInstaller, InstallerType, Package};

use super::{EngineKind, msix, portable, zip};

type InstallFn = fn(&CatalogInstaller, &Path, &Path) -> Result<()>;
type RemoveFn = fn(&Package) -> Result<()>;
type MatchesInstallerFn = fn(&CatalogInstaller) -> bool;
type MatchesKindFn = fn(&str) -> bool;

struct EngineDescriptor {
    kind: EngineKind,
    install: InstallFn,
    remove: RemoveFn,
    matches_installer: MatchesInstallerFn,
    matches_kind: MatchesKindFn,
}

fn matches_msix_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Msix
}

fn matches_msix_kind(kind: &str) -> bool {
    kind.trim().eq_ignore_ascii_case("msix")
}

fn matches_zip_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Zip
        || (installer.kind == InstallerType::Portable && is_zip_path(&installer.url))
}

fn matches_zip_kind(kind: &str) -> bool {
    kind.trim().eq_ignore_ascii_case("zip")
}

fn matches_portable_installer(installer: &CatalogInstaller) -> bool {
    installer.kind == InstallerType::Portable
}

fn matches_portable_kind(kind: &str) -> bool {
    kind.trim().eq_ignore_ascii_case("portable")
}

fn msix_install(
    _installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
) -> Result<()> {
    msix::install::install(download_path, install_dir)
}

fn zip_install(
    _installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
) -> Result<()> {
    zip::install::install(download_path, install_dir)
}

fn portable_install(
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
) -> Result<()> {
    portable::install::install(download_path, install_dir, &installer.url)
}

fn msix_remove(package: &Package) -> Result<()> {
    msix::remove::remove(package)
}

fn zip_remove(package: &Package) -> Result<()> {
    zip::remove::remove(package)
}

fn portable_remove(package: &Package) -> Result<()> {
    portable::remove::remove(package)
}

const ENGINE_DESCRIPTORS: &[EngineDescriptor] = &[
    EngineDescriptor {
        kind: EngineKind::Msix,
        install: msix_install,
        remove: msix_remove,
        matches_installer: matches_msix_installer,
        matches_kind: matches_msix_kind,
    },
    EngineDescriptor {
        kind: EngineKind::Zip,
        install: zip_install,
        remove: zip_remove,
        matches_installer: matches_zip_installer,
        matches_kind: matches_zip_kind,
    },
    EngineDescriptor {
        kind: EngineKind::Portable,
        install: portable_install,
        remove: portable_remove,
        matches_installer: matches_portable_installer,
        matches_kind: matches_portable_kind,
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

pub(crate) fn resolve_engine_kind_for_kind(kind: &str) -> Result<EngineKind> {
    ENGINE_DESCRIPTORS
        .iter()
        .find(|descriptor| (descriptor.matches_kind)(kind))
        .map(|descriptor| descriptor.kind)
        .ok_or_else(|| anyhow!("unsupported installer type '{}'", kind.trim()))
}

pub(crate) fn install(
    kind: EngineKind,
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
) -> Result<()> {
    let descriptor = resolve_engine_descriptor(kind)?;

    (descriptor.install)(installer, download_path, install_dir)
}

pub(crate) fn remove(kind: EngineKind, package: &Package) -> Result<()> {
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
    use super::{resolve_engine_kind_for_installer, resolve_engine_kind_for_kind};
    use crate::engines::EngineKind;
    use crate::models::CatalogInstaller;

    fn installer(kind: &str, url: &str) -> CatalogInstaller {
        CatalogInstaller {
            package_id: "Contoso.App".to_string(),
            url: url.to_string(),
            hash: "hash".to_string(),
            arch: "x64".parse().expect("arch should parse"),
            kind: kind.parse().expect("kind should parse"),
        }
    }

    #[test]
    fn resolve_installer_treats_portable_zip_as_zip() {
        let engine = resolve_engine_kind_for_installer(&installer(
            "portable",
            "https://example.invalid/tool.zip",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Zip);
    }

    #[test]
    fn resolve_kind_returns_portable() {
        let engine = resolve_engine_kind_for_kind("portable").expect("engine kind should resolve");

        assert_eq!(engine, EngineKind::Portable);
    }

    #[test]
    fn resolve_kind_rejects_unknown_type() {
        let err = resolve_engine_kind_for_kind("exe").expect_err("unknown type should fail");

        assert!(err.to_string().contains("unsupported installer type 'exe'"));
    }

    #[test]
    fn resolve_installer_prefers_msix_for_msix_kind() {
        let engine = resolve_engine_kind_for_installer(&installer(
            "  msix  ",
            "https://example.invalid/package.msix",
        ))
        .expect("engine should resolve");

        assert_eq!(engine, EngineKind::Msix);
    }
}
