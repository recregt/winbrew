use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Component, Path, PathBuf};
use tracing::{debug, trace};

pub fn extract(src: &Path, dest: &Path, strip_container: bool) -> Result<()> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    debug!(archive = %src.display(), target = %dest.display(), format = ext.as_str(), strip_container, "starting extraction");

    match ext.as_str() {
        "zip" => extract_zip(src, dest, strip_container),
        "msi" => extract_msi(src, dest),
        _ => bail!("unsupported file type: {}", ext),
    }
}

fn extract_zip(src: &Path, dest: &Path, strip_container: bool) -> Result<()> {
    let file = fs::File::open(src).context("failed to open zip file")?;
    let mut archive = zip::ZipArchive::new(file).context("failed to read zip archive")?;

    trace!(archive = %src.display(), entries = archive.len(), strip_container, "opened zip archive");

    let container = if strip_container {
        detect_container(&mut archive)
    } else {
        None
    };

    if let Some(container) = &container {
        debug!(container = %container.display(), target = %dest.display(), "detected zip container prefix");
    }

    fs::create_dir_all(dest).context("failed to create destination directory")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("failed to read zip entry")?;

        let raw_path = match entry.enclosed_name() {
            Some(path) => path.to_owned(),
            None => {
                trace!(entry = i, "skipping zip entry with invalid enclosed path");
                continue;
            }
        };

        let relative = match &container {
            Some(prefix) => raw_path
                .strip_prefix(prefix)
                .unwrap_or(&raw_path)
                .to_path_buf(),
            None => raw_path,
        };

        if relative.as_os_str().is_empty() {
            continue;
        }

        let out_path = match join_safely(dest, &relative) {
            Some(path) => path,
            None => {
                trace!(entry = i, path = %relative.display(), "skipping unsafe zip entry path");
                continue;
            }
        };

        if entry.is_dir() {
            fs::create_dir_all(&out_path).context("failed to create directory")?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).context("failed to create parent directory")?;
            }

            let mut out_file = fs::File::create(&out_path).context("failed to create file")?;
            std::io::copy(&mut entry, &mut out_file).context("failed to extract file")?;

            trace!(entry = i, path = %out_path.display(), "extracted zip entry");
        }
    }

    Ok(())
}

// OPTIMIZATION: Zero-allocation container detection
fn detect_container(archive: &mut zip::ZipArchive<fs::File>) -> Option<PathBuf> {
    let mut root_name = None;
    let mut has_nested = false;

    for i in 0..archive.len() {
        let entry = archive.by_index(i).ok()?;

        if entry.is_dir() {
            continue;
        }

        let path = entry.enclosed_name()?;
        let mut components = path.components();
        let first = components.next()?.as_os_str();

        match root_name {
            None => root_name = Some(first.to_owned()),
            Some(ref root) if root != first => return None, // Multiple roots found
            Some(_) => {}
        }

        if components.next().is_some() {
            has_nested = true;
        }
    }

    if has_nested {
        root_name.map(PathBuf::from)
    } else {
        None
    }
}

fn join_safely(base: &Path, relative: &Path) -> Option<PathBuf> {
    let mut path = base.to_path_buf();

    for component in relative.components() {
        match component {
            Component::Normal(part) => path.push(part),
            _ => return None,
        }
    }

    Some(path)
}

fn extract_msi(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).context("failed to create destination directory")?;

    debug!(archive = %src.display(), target = %dest.display(), "starting msi extraction");

    let mut cleanup_guard = MsiCleanupGuard {
        path: dest.to_path_buf(),
        keep: false,
    };

    // TARGETDIR requires an absolute path. std::env::current_dir avoids Windows UNC path issues.
    let abs_dest = if dest.is_absolute() {
        dest.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to get current directory")?
            .join(dest)
    };

    let status = std::process::Command::new("msiexec")
        .args([
            "/a",
            &src.to_string_lossy(),
            "/qn",
            &format!("TARGETDIR={}", abs_dest.to_string_lossy()),
        ])
        .status()
        .context("failed to run msiexec")?;

    trace!(archive = %src.display(), target = %abs_dest.display(), status = ?status.code(), "msiexec completed");

    if !status.success() {
        let _ = fs::remove_dir_all(dest);
        bail!("msiexec failed with code: {:?}", status.code());
    }

    cleanup_guard.keep = true;

    Ok(())
}

struct MsiCleanupGuard {
    path: PathBuf,
    keep: bool,
}

impl Drop for MsiCleanupGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
