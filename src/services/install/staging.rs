use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::models::CatalogInstaller;

use super::download::{installer_filename, is_zip_path};

pub fn stage_installer(
    installer: &CatalogInstaller,
    download_path: &Path,
    stage_dir: &Path,
    package_name: &str,
) -> Result<()> {
    cleanup_path(stage_dir)?;
    fs::create_dir_all(stage_dir)
        .with_context(|| format!("failed to create staging directory {}", stage_dir.display()))?;

    let installer_kind = installer.kind.trim().to_ascii_lowercase();
    if installer_kind == "zip" || (installer_kind == "portable" && is_zip_path(&installer.url)) {
        extract_zip_archive(download_path, stage_dir)?;
        return Ok(());
    }

    if installer_kind == "portable" {
        let file_name = installer_filename(&installer.url);
        let target_path = stage_dir.join(file_name);
        fs::copy(download_path, &target_path).with_context(|| {
            format!(
                "failed to stage portable installer into {}",
                target_path.display()
            )
        })?;
        return Ok(());
    }

    if installer_kind == "msix" {
        run_msix_installer(download_path)?;
        let target_path = stage_dir.join(format!("{package_name}.msix"));
        fs::copy(download_path, &target_path).with_context(|| {
            format!(
                "failed to stage msix installer into {}",
                target_path.display()
            )
        })?;
        return Ok(());
    }

    bail!("unsupported installer type: {}", installer.kind)
}

pub fn replace_directory(source_dir: &Path, target_dir: &Path) -> Result<()> {
    if target_dir.exists() {
        cleanup_path(target_dir)?;
    }

    fs::rename(source_dir, target_dir).with_context(|| {
        format!(
            "failed to move staged installation into place: {} -> {}",
            source_dir.display(),
            target_dir.display()
        )
    })?;

    Ok(())
}

pub fn cleanup_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

fn run_msix_installer(path: &Path) -> Result<()> {
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!("Add-AppxPackage -Path '{}'", path.display()),
        ])
        .status()
        .context("failed to start PowerShell for msix installation")?;

    if !status.success() {
        bail!("msix install failed with code: {:?}", status.code());
    }

    Ok(())
}

fn extract_zip_archive(zip_path: &Path, destination_dir: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("failed to open zip archive {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("failed to open zip archive")?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("failed to read zip entry")?;
        let enclosed_name = entry
            .enclosed_name()
            .ok_or_else(|| anyhow::anyhow!("zip entry contains an invalid path"))?;
        let outpath = destination_dir.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&outpath).with_context(|| {
                format!("failed to create extracted directory {}", outpath.display())
            })?;
            continue;
        }

        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create parent directory {}", parent.display())
            })?;
        }

        let mut outfile = fs::File::create(&outpath)
            .with_context(|| format!("failed to create extracted file {}", outpath.display()))?;
        std::io::copy(&mut entry, &mut outfile)
            .with_context(|| format!("failed to extract {}", outpath.display()))?;
    }

    Ok(())
}
