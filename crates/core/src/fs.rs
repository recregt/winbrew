use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

static DEFERRED_DELETE_SUFFIX: AtomicUsize = AtomicUsize::new(0);

pub fn cleanup_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let removal_result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };

    match removal_result {
        Ok(()) => Ok(()),
        Err(err) => {
            if let Some(deferred_path) = deferred_delete_path(path) {
                if deferred_path.exists() {
                    let _ = cleanup_path(&deferred_path);
                }

                if fs::rename(path, &deferred_path).is_ok() {
                    return Ok(());
                }

                return Err(err).with_context(|| {
                    format!(
                        "failed to remove {} and defer deletion to {}",
                        path.display(),
                        deferred_path.display()
                    )
                });
            }

            Err(err).with_context(|| format!("failed to remove {}", path.display()))
        }
    }
}

pub fn atomic_write(path: &Path, temp_path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory for {}", path.display()))?;
    }

    {
        let mut file = fs::File::create(temp_path)
            .with_context(|| format!("failed to create temp file at {}", temp_path.display()))?;
        file.write_all(contents)
            .with_context(|| format!("failed to write temp file at {}", temp_path.display()))?;
        file.flush()
            .with_context(|| format!("failed to flush temp file at {}", temp_path.display()))?;
    }

    if let Err(err) = fs::rename(temp_path, path) {
        let _ = fs::remove_file(temp_path);
        return Err(err).with_context(|| {
            format!(
                "failed to finalize atomic write: {} -> {}",
                temp_path.display(),
                path.display()
            )
        });
    }

    Ok(())
}

pub fn atomic_write_with_pid_suffix(path: &Path, contents: &str) -> Result<()> {
    let temp_path = path.with_extension(format!("toml.{}.tmp", process::id()));
    atomic_write(path, &temp_path, contents.as_bytes())
}

pub fn finalize_temp_file(temp_path: &Path, final_path: &Path) -> Result<()> {
    if final_path.exists() {
        cleanup_path(final_path)?;
    }

    fs::rename(temp_path, final_path).with_context(|| {
        format!(
            "failed to finalize file: {} -> {}",
            temp_path.display(),
            final_path.display()
        )
    })
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

    let backup_dir = backup_directory_path(target_dir);
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
        return Err(err);
    }

    let _ = cleanup_path(&backup_dir);

    Ok(())
}

pub fn extract_zip_archive(zip_path: &Path, destination_dir: &Path) -> Result<()> {
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

pub fn backup_directory_path(target_dir: &Path) -> PathBuf {
    let parent = target_dir.parent().unwrap_or(target_dir);
    let name = target_dir
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_default();

    parent.join(format!("{name}.old"))
}

fn deferred_delete_path(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy();
    let suffix = DEFERRED_DELETE_SUFFIX.fetch_add(1, Ordering::Relaxed);

    Some(path.with_file_name(format!("{file_name}.deleted.{}.{}", process::id(), suffix)))
}

#[cfg(test)]
mod tests {
    use super::backup_directory_path;
    use std::path::Path;

    #[test]
    fn backup_directory_path_appends_old_suffix_next_to_target() {
        let path = Path::new(r"C:\pkg\tool.exe");
        assert_eq!(
            backup_directory_path(path),
            Path::new(r"C:\pkg\tool.exe.old")
        );
    }
}
