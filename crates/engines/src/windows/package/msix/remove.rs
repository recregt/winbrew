use anyhow::{Context, Result, bail};

use winbrew_models::{EngineMetadata, InstalledPackage as WinbrewPackage};

#[cfg(windows)]
use winbrew_windows::msix_remove;

pub fn remove(package: &WinbrewPackage) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = package;
        bail!("MSIX removal is only supported on Windows")
    }

    #[cfg(windows)]
    {
        let package_full_name = match package.engine_metadata.as_ref() {
            Some(EngineMetadata::Msix {
                package_full_name, ..
            }) => package_full_name,
            _ => bail!("missing msix receipt metadata for '{}'", package.name),
        };

        msix_remove(package_full_name)
            .with_context(|| format!("msix uninstall failed for {package_full_name}"))?;

        Ok(())
    }
}
