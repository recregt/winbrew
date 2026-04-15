#[cfg(windows)]
pub mod msix;

#[cfg(not(windows))]
pub mod msix {
    pub mod install {
        use anyhow::{Result, bail};
        use std::path::Path;

        use winbrew_models::install::engine::EngineInstallReceipt;

        pub fn install(
            _download_path: &Path,
            _install_dir: &Path,
            _package_name: &str,
        ) -> Result<EngineInstallReceipt> {
            bail!("MSIX installation is only supported on Windows")
        }
    }

    pub mod remove {
        use anyhow::{Result, bail};

        use winbrew_models::install::installed::InstalledPackage;

        pub fn remove(_package: &InstalledPackage) -> Result<()> {
            bail!("MSIX removal is only supported on Windows")
        }
    }
}
