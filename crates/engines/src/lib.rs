//! Engine dispatch and platform-specific installers for WinBrew.
//!
//! `winbrew-engines` maps installer metadata to execution backends and owns
//! the filesystem and Windows-specific install/remove implementations. The
//! crate stays focused on engine selection, engine receipts, and platform
//! adapters so the app layer can orchestrate without embedding OS details.

mod payload;
mod registry;

pub mod filesystem;
pub mod windows;

pub use filesystem::archive::zip;
pub use filesystem::portable;
pub use windows::package::msix;

use anyhow::Result;
use std::path::Path;

pub use winbrew_models::install::engine::{EngineInstallReceipt, EngineKind};
pub use winbrew_models::shared::DeploymentKind;

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

pub fn resolve_deployment_kind(installer: &CatalogInstaller) -> DeploymentKind {
    registry::resolve_deployment_kind(installer)
}

pub fn engine_kind_for_type(kind: InstallerType) -> Result<EngineKind> {
    Ok(EngineKind::from_installer_type(kind))
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
    use super::{DeploymentKind, EngineKind, engine_kind_for_type, resolve_deployment_kind};
    use winbrew_models::catalog::package::CatalogInstaller;
    use winbrew_models::install::installer::InstallerType;

    fn installer(kind: InstallerType, nested_kind: Option<InstallerType>) -> CatalogInstaller {
        CatalogInstaller {
            package_id: "Contoso.App".into(),
            url: "https://example.invalid/app.zip".to_string(),
            hash: "hash".to_string(),
            arch: "x64".parse().expect("arch should parse"),
            kind,
            nested_kind,
        }
    }

    #[test]
    fn engine_kind_for_type_maps_supported_types() {
        assert_eq!(
            engine_kind_for_type(InstallerType::Msi).unwrap(),
            EngineKind::Msi
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Appx).unwrap(),
            EngineKind::Msix
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Msix).unwrap(),
            EngineKind::Msix
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Wix).unwrap(),
            EngineKind::Msi
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Zip).unwrap(),
            EngineKind::Zip
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Portable).unwrap(),
            EngineKind::Portable
        );
        assert_eq!(
            engine_kind_for_type(InstallerType::Exe).unwrap(),
            EngineKind::NativeExe
        );
    }

    #[test]
    fn engine_kind_for_type_recognizes_native_exe_family() {
        assert_eq!(
            engine_kind_for_type(InstallerType::Inno).unwrap(),
            EngineKind::NativeExe
        );
    }

    #[test]
    fn resolve_deployment_kind_uses_nested_installer_type_for_archives() {
        let installer = installer(InstallerType::Zip, Some(InstallerType::Msi));

        assert_eq!(
            resolve_deployment_kind(&installer),
            DeploymentKind::Installed
        );
    }
}
