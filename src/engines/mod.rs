mod registry;

pub mod msix;
pub mod portable;
pub mod zip;

use anyhow::Result;
use std::path::Path;

use crate::models::CatalogInstaller;
use crate::models::InstallerType;
use crate::models::Package;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EngineKind {
    Msix,
    Zip,
    Portable,
}

pub(crate) trait PackageEngine {
    fn install(
        &self,
        installer: &CatalogInstaller,
        download_path: &Path,
        install_dir: &Path,
    ) -> Result<()>;

    fn remove(&self, package: &Package) -> Result<()>;
}

pub(crate) fn get_engine(installer: &CatalogInstaller) -> Result<EngineKind> {
    registry::resolve_engine_kind_for_installer(installer)
}

pub(crate) fn get_engine_kind(kind: InstallerType) -> Result<EngineKind> {
    match kind {
        InstallerType::Msix => Ok(EngineKind::Msix),
        InstallerType::Zip => Ok(EngineKind::Zip),
        InstallerType::Portable => Ok(EngineKind::Portable),
        other => Err(anyhow::anyhow!(
            "unsupported installer type '{}'",
            other.as_str()
        )),
    }
}

impl PackageEngine for EngineKind {
    fn install(
        &self,
        installer: &CatalogInstaller,
        download_path: &Path,
        install_dir: &Path,
    ) -> Result<()> {
        registry::install(*self, installer, download_path, install_dir)
    }

    fn remove(&self, package: &Package) -> Result<()> {
        registry::remove(*self, package)
    }
}
