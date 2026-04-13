//! Engine dispatch and platform-specific installers for WinBrew.
//!
//! `winbrew-engines` maps installer metadata to execution backends and owns
//! the filesystem and Windows-specific install/remove implementations. The
//! crate stays focused on engine selection, engine receipts, and platform
//! adapters so the app layer can orchestrate without embedding OS details.

mod registry;

pub mod filesystem;
pub mod windows;

pub use filesystem::archive::zip;
pub use filesystem::portable;
pub use windows::package::msix;

use anyhow::Result;
use std::path::Path;

pub use winbrew_models::install::engine::{EngineInstallReceipt, EngineKind};

use winbrew_models::catalog::package::CatalogInstaller;
use winbrew_models::install::installed::InstalledPackage;
use winbrew_models::install::installer::InstallerType;

pub trait PackageEngine {
    fn install(
        &self,
        installer: &CatalogInstaller,
        download_path: &Path,
        install_dir: &Path,
        package_name: &str,
    ) -> Result<EngineInstallReceipt>;

    fn remove(&self, package: &InstalledPackage) -> Result<()>;
}

pub fn resolve_engine_for_installer(installer: &CatalogInstaller) -> Result<EngineKind> {
    registry::resolve_engine_kind_for_installer(installer)
}

pub fn engine_kind_for_type(kind: InstallerType) -> Result<EngineKind> {
    match kind {
        InstallerType::Msi => Ok(EngineKind::Msi),
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
        package_name: &str,
    ) -> Result<EngineInstallReceipt> {
        registry::install(*self, installer, download_path, install_dir, package_name)
    }

    fn remove(&self, package: &InstalledPackage) -> Result<()> {
        registry::remove(*self, package)
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineKind, engine_kind_for_type};
    use winbrew_models::install::installer::InstallerType;

    #[test]
    fn engine_kind_for_type_maps_supported_types() {
        assert_eq!(
            engine_kind_for_type(InstallerType::Msi).unwrap(),
            EngineKind::Msi
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Msix).unwrap(),
            EngineKind::Msix
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Zip).unwrap(),
            EngineKind::Zip
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Portable).unwrap(),
            EngineKind::Portable
        );
    }

    #[test]
    fn engine_kind_for_type_rejects_exe() {
        let err =
            engine_kind_for_type(InstallerType::Exe).expect_err("exe should not map to an engine");

        assert!(err.to_string().contains("unsupported installer type 'exe'"));
    }
}
