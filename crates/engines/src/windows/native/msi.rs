use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, warn};

use winbrew_models::{
    EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope, InstalledPackage,
};
use winbrew_windows::{msi_scan_inventory, uninstall_value};

const MSI_INSTALL_EXIT_CODES: &[i32] = &[0, 1641, 3010];
const INSTALL_LOCATION_VALUE: &str = "InstallLocation";

/// Install an MSI package through Windows Installer and capture MSI metadata.
///
/// The helper scans the downloaded MSI database first so the engine metadata
/// can preserve the product code, upgrade code, and the stable registry/
/// shortcut references stored in the package database. The actual installation
/// then runs through `msiexec` in silent mode.
pub fn install(
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    let snapshot = msi_scan_inventory(
        download_path,
        install_dir,
        package_name,
        InstallScope::Installed,
    )
    .with_context(|| format!("failed to scan MSI inventory for {}", package_name))?;

    fs::create_dir_all(install_dir)
        .with_context(|| format!("failed to create {}", install_dir.display()))?;

    let status = Command::new("msiexec")
        .arg("/i")
        .arg(download_path)
        .arg(format!(r"TARGETDIR={}", install_dir.display()))
        .arg(format!(r"INSTALLDIR={}", install_dir.display()))
        .arg("/qn")
        .arg("/norestart")
        .status()
        .context("failed to launch msiexec for MSI install")?;

    let exit_code = status
        .code()
        .ok_or_else(|| anyhow::anyhow!("msiexec terminated without an exit code"))?;

    if !MSI_INSTALL_EXIT_CODES.contains(&exit_code) {
        bail!(
            "msiexec failed for {} with exit code {}",
            package_name,
            exit_code
        );
    }

    let install_dir = resolve_install_dir(&snapshot, install_dir, package_name);

    let snapshot_receipt = snapshot.receipt.clone();
    let registry_keys = collect_registry_keys(&snapshot);
    let shortcuts = collect_shortcuts(&snapshot);

    let engine_metadata = Some(EngineMetadata::Msi {
        product_code: snapshot_receipt.product_code,
        upgrade_code: snapshot_receipt.upgrade_code,
        scope: snapshot_receipt.scope,
        registry_keys,
        shortcuts,
    });

    let mut receipt = EngineInstallReceipt::new(
        EngineKind::Msi,
        install_dir.to_string_lossy().into_owned(),
        engine_metadata,
    );
    receipt.msi_inventory_snapshot = Some(snapshot);

    Ok(receipt)
}

/// Remove an installed MSI package using the product code stored in metadata.
pub fn remove(package: &InstalledPackage) -> Result<()> {
    let product_code = match package.engine_metadata.as_ref() {
        Some(EngineMetadata::Msi { product_code, .. }) => product_code.as_str(),
        _ => bail!("missing MSI receipt metadata for '{}'", package.name),
    };

    let status = Command::new("msiexec")
        .arg("/x")
        .arg(product_code)
        .arg("/qn")
        .arg("/norestart")
        .status()
        .context("failed to launch msiexec for MSI removal")?;

    let exit_code = status
        .code()
        .ok_or_else(|| anyhow::anyhow!("msiexec terminated without an exit code"))?;

    if !MSI_INSTALL_EXIT_CODES.contains(&exit_code) {
        bail!(
            "msiexec removal failed for {} with exit code {}",
            package.name,
            exit_code
        );
    }

    Ok(())
}

fn collect_registry_keys(snapshot: &winbrew_models::MsiInventorySnapshot) -> Vec<String> {
    let mut registry_keys = snapshot
        .registry_entries
        .iter()
        .map(|entry| format!(r"{}\{}", entry.hive, entry.key_path))
        .collect::<Vec<_>>();

    registry_keys.sort_unstable();
    registry_keys.dedup();

    registry_keys
}

fn collect_shortcuts(snapshot: &winbrew_models::MsiInventorySnapshot) -> Vec<String> {
    let mut shortcuts = snapshot
        .shortcuts
        .iter()
        .map(|shortcut| shortcut.path.clone())
        .collect::<Vec<_>>();

    shortcuts.sort_unstable();
    shortcuts.dedup();

    shortcuts
}

fn resolve_install_dir(
    snapshot: &winbrew_models::MsiInventorySnapshot,
    requested_install_dir: &Path,
    package_name: &str,
) -> PathBuf {
    match uninstall_value(&snapshot.receipt.product_code, INSTALL_LOCATION_VALUE) {
        Some(install_location) => {
            let actual_install_dir = PathBuf::from(&install_location);

            if !same_install_dir(&actual_install_dir, requested_install_dir) {
                warn!(
                    package = package_name,
                    product_code = %snapshot.receipt.product_code,
                    requested_install_dir = %requested_install_dir.display(),
                    registry_install_location = %install_location,
                    "MSI InstallLocation differs from the path used to build the scan snapshot"
                );

                debug!(
                    package = package_name,
                    product_code = %snapshot.receipt.product_code,
                    file_count = snapshot.files.len(),
                    registry_entry_count = snapshot.registry_entries.len(),
                    shortcut_count = snapshot.shortcuts.len(),
                    component_count = snapshot.components.len(),
                    "MSI inventory details for InstallLocation mismatch"
                );
            }

            actual_install_dir
        }
        None => {
            warn!(
                package = package_name,
                product_code = %snapshot.receipt.product_code,
                requested_install_dir = %requested_install_dir.display(),
                file_count = snapshot.files.len(),
                "MSI InstallLocation was not published; using the requested install directory"
            );

            debug!(
                package = package_name,
                product_code = %snapshot.receipt.product_code,
                registry_entry_count = snapshot.registry_entries.len(),
                shortcut_count = snapshot.shortcuts.len(),
                component_count = snapshot.components.len(),
                "MSI inventory details for missing InstallLocation"
            );

            requested_install_dir.to_path_buf()
        }
    }
}

fn same_install_dir(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => normalize_install_dir_text(left) == normalize_install_dir_text(right),
    }
}

fn normalize_install_dir_text(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}
