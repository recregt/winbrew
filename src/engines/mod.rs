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

#[cfg(test)]
mod tests {
    use super::{EngineKind, get_engine_kind};
    use crate::models::InstallerType;

    #[test]
    fn get_engine_kind_maps_supported_types() {
        assert_eq!(
            get_engine_kind(InstallerType::Msix).unwrap(),
            EngineKind::Msix
        );
        assert_eq!(
            get_engine_kind(InstallerType::Zip).unwrap(),
            EngineKind::Zip
        );
        assert_eq!(
            get_engine_kind(InstallerType::Portable).unwrap(),
            EngineKind::Portable
        );
    }

    #[test]
    fn get_engine_kind_rejects_exe() {
        let err = get_engine_kind(InstallerType::Exe).expect_err("exe should not map to an engine");

        assert!(err.to_string().contains("unsupported installer type 'exe'"));
    }
}
