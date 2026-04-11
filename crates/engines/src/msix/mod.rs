#[cfg(windows)]
pub mod install;
#[cfg(not(windows))]
pub mod install {
    use anyhow::{Result, bail};
    use std::path::Path;

    use winbrew_models::EngineInstallReceipt;

    pub fn install(
        _download_path: &Path,
        _install_dir: &Path,
        _package_name: &str,
    ) -> Result<EngineInstallReceipt> {
        bail!("MSIX installation is only supported on Windows")
    }
}

#[cfg(windows)]
pub mod remove;
#[cfg(not(windows))]
pub mod remove {
    use anyhow::{Result, bail};

    use winbrew_models::InstalledPackage;

    pub fn remove(_package: &InstalledPackage) -> Result<()> {
        bail!("MSIX removal is only supported on Windows")
    }
}

use anyhow::{Result, bail};

#[cfg(windows)]
use anyhow::Context;

#[cfg(windows)]
use windows::Management::Deployment::PackageManager;

pub fn installed_package_full_name(package_name: &str) -> Result<String> {
    #[cfg(not(windows))]
    {
        let _ = package_name;
        bail!("MSIX package lookup is only supported on Windows")
    }

    #[cfg(windows)]
    {
        let package_manager = PackageManager::new().context("failed to create package manager")?;
        let matching_full_names =
            remove::matching_package_full_names(&package_manager, package_name)?;

        match matching_full_names.as_slice() {
            [full_name] => Ok(full_name.to_string()),
            [] => bail!("no installed msix package matched '{package_name}'"),
            _ => bail!("multiple installed msix packages matched '{package_name}'"),
        }
    }
}
