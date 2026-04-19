use anyhow::{Context, Result};
use std::path::Path;

use tracing::warn;

use crate::core::fs::cleanup_path;
use crate::models::catalog::package::CatalogInstaller;
use crate::models::install::engine::{EngineInstallReceipt, EngineKind};
use crate::models::install::installed::InstalledPackage;

#[cfg(windows)]
use crate::windows_dep::{install_user_font, remove_user_font};

/// Install a per-user font by copying the downloaded font file into the
/// Windows user fonts directory.
pub fn install(
    _installer: &CatalogInstaller,
    download_path: &Path,
    _install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    #[cfg(not(windows))]
    {
        let _ = (_installer, download_path, package_name);
        anyhow::bail!("font installation is only supported on Windows")
    }

    #[cfg(windows)]
    {
        let installed_path = install_user_font(download_path)
            .with_context(|| format!("failed to install font package for {package_name}"))?;

        Ok(EngineInstallReceipt::new(
            EngineKind::Font,
            installed_path.to_string_lossy().into_owned(),
            None,
        ))
    }
}

/// Remove a per-user font file and its backing filesystem registration.
pub fn remove(package: &InstalledPackage) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = package;
        anyhow::bail!("font removal is only supported on Windows")
    }

    #[cfg(windows)]
    {
        if let Err(err) = remove_user_font(Path::new(&package.install_dir)) {
            warn!(
                package = package.name.as_str(),
                error = %err,
                "font removal helper reported an error; continuing with filesystem cleanup"
            );
        }

        let _ = cleanup_path(Path::new(&package.install_dir));

        Ok(())
    }
}
