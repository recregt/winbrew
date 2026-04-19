//! MSIX removal implementation.
//!
//! This adapter expects the installed package receipt to contain
//! `EngineMetadata::Msix`. It extracts the stored package full name and passes
//! it to `crate::windows_dep::msix_remove`.
//!
//! The module does not query the registry or derive package identity on its
//! own. That information must already be present in the receipt.

use anyhow::{Context, Result, bail};

use crate::models::install::engine::EngineMetadata;
use crate::models::install::installed::InstalledPackage as WinbrewPackage;

use crate::windows_dep::msix_remove;

/// Remove an MSIX package using the package full name stored in the receipt.
///
/// Returns an error when the installed package does not carry MSIX metadata or
/// when Windows rejects the uninstall call.
pub fn remove(package: &WinbrewPackage) -> Result<()> {
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
