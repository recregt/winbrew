pub mod msix;
pub mod portable;
pub mod zip;

use anyhow::{Result, bail};
use std::path::Path;

use crate::core::network::is_zip_path;
use crate::models::CatalogInstaller;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    Msix,
    Zip,
    Portable,
}

pub trait PackageEngine {
    fn install(
        &self,
        installer: &CatalogInstaller,
        download_path: &Path,
        install_dir: &Path,
    ) -> Result<()>;

    fn remove(
        &self,
        package_name: &str,
        install_dir: &Path,
        msix_package_full_name: Option<&str>,
    ) -> Result<()>;
}

pub fn get_engine(installer: &CatalogInstaller) -> Result<EngineKind> {
    let installer_kind = installer.kind.trim().to_ascii_lowercase();

    match installer_kind.as_str() {
        "msix" => Ok(EngineKind::Msix),
        "zip" => Ok(EngineKind::Zip),
        "portable" if is_zip_path(&installer.url) => Ok(EngineKind::Zip),
        "portable" => Ok(EngineKind::Portable),
        _ => bail!("unsupported installer type: {}", installer.kind),
    }
}

pub fn get_engine_kind(kind: &str) -> Result<EngineKind> {
    let installer_kind = kind.trim().to_ascii_lowercase();

    match installer_kind.as_str() {
        "msix" => Ok(EngineKind::Msix),
        "zip" => Ok(EngineKind::Zip),
        "portable" => Ok(EngineKind::Portable),
        _ => bail!("unsupported installer type: {}", kind),
    }
}

impl PackageEngine for EngineKind {
    fn install(
        &self,
        installer: &CatalogInstaller,
        download_path: &Path,
        install_dir: &Path,
    ) -> Result<()> {
        match self {
            EngineKind::Msix => msix::install::install(download_path, install_dir),
            EngineKind::Zip => zip::install::install(download_path, install_dir),
            EngineKind::Portable => {
                portable::install::install(download_path, install_dir, &installer.url)
            }
        }
    }

    fn remove(
        &self,
        package_name: &str,
        install_dir: &Path,
        msix_package_full_name: Option<&str>,
    ) -> Result<()> {
        match self {
            EngineKind::Msix => {
                msix::remove::remove(package_name, install_dir, msix_package_full_name)
            }
            EngineKind::Zip => {
                zip::remove::remove(package_name, install_dir, msix_package_full_name)
            }
            EngineKind::Portable => {
                portable::remove::remove(package_name, install_dir, msix_package_full_name)
            }
        }
    }
}
