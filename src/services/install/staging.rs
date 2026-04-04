use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::core::network::{installer_filename, is_zip_path};
use crate::models::CatalogInstaller;

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
        let target_path = stage_dir.join(msix_filename(package_name));
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
    if !target_dir.exists() {
        fs::rename(source_dir, target_dir).with_context(|| {
            format!(
                "failed to move staged installation into place: {} -> {}",
                source_dir.display(),
                target_dir.display()
            )
        })?;

        return Ok(());
    }

    let backup_dir = target_dir.with_extension("old");
    cleanup_path(&backup_dir)?;

    fs::rename(target_dir, &backup_dir).with_context(|| {
        format!(
            "failed to move existing installation aside: {} -> {}",
            target_dir.display(),
            backup_dir.display()
        )
    })?;

    let rename_result = fs::rename(source_dir, target_dir).with_context(|| {
        format!(
            "failed to move staged installation into place: {} -> {}",
            source_dir.display(),
            target_dir.display()
        )
    });

    if let Err(err) = rename_result {
        let _ = fs::rename(&backup_dir, target_dir);
        let _ = cleanup_path(&backup_dir);
        return Err(err);
    }

    let _ = cleanup_path(&backup_dir);

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
    let command = powershell_add_appx_command(path);

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &command,
        ])
        .status()
        .context("failed to start PowerShell for msix installation")?;

    if !status.success() {
        bail!("msix install failed with code: {:?}", status.code());
    }

    Ok(())
}

fn msix_filename(package_name: &str) -> String {
    let mut filename = String::with_capacity(package_name.len() + 5);
    filename.push_str(package_name);
    filename.push_str(".msix");
    filename
}

fn powershell_add_appx_command(path: &Path) -> String {
    let path = path.display().to_string();
    let mut command = String::with_capacity("Add-AppxPackage -Path ''".len() + path.len());
    command.push_str("Add-AppxPackage -Path '");
    command.push_str(&path);
    command.push('\'');
    command
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
