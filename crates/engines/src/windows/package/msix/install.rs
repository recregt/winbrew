use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use winbrew_models::{EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope};

#[cfg(windows)]
use winbrew_windows::msix_install;

pub fn install(
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    #[cfg(not(windows))]
    {
        let _ = (download_path, install_dir, package_name);
        anyhow::bail!("MSIX installation is only supported on Windows")
    }

    #[cfg(windows)]
    {
        let package_full_name =
            msix_install(download_path, package_name).context("msix install failed")?;

        fs::create_dir_all(install_dir)
            .with_context(|| format!("failed to create {}", install_dir.display()))?;

        let engine_metadata = Some(EngineMetadata::msix(
            package_full_name,
            InstallScope::Installed,
        ));

        Ok(EngineInstallReceipt::new(EngineKind::Msix, engine_metadata))
    }
}
