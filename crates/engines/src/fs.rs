use anyhow::{Context, Result};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

static DEFERRED_DELETE_SUFFIX: AtomicUsize = AtomicUsize::new(0);

pub fn cleanup_path(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };

    let removal_result = if is_reparse_point(&metadata) {
        fs::remove_dir(path).or_else(|_| fs::remove_file(path))
    } else if metadata.is_dir() {
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

/// Replaces `source_dir` with `target_dir`, rolling back the backup on failure.
pub fn replace_directory(source_dir: &Path, target_dir: &Path) -> Result<()> {
    replace_directory_with_rename(source_dir, target_dir, rename_path)
}

fn replace_directory_with_rename<R>(source_dir: &Path, target_dir: &Path, rename: R) -> Result<()>
where
    R: Fn(&Path, &Path) -> std::io::Result<()>,
{
    if !target_dir.exists() {
        rename(source_dir, target_dir).with_context(|| {
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

    rename(target_dir, &backup_dir).with_context(|| {
        format!(
            "failed to move existing installation aside: {} -> {}",
            target_dir.display(),
            backup_dir.display()
        )
    })?;

    let rename_result = rename(source_dir, target_dir).with_context(|| {
        format!(
            "failed to move staged installation into place: {} -> {}",
            source_dir.display(),
            target_dir.display()
        )
    });

    if let Err(err) = rename_result {
        if let Err(rollback_err) = rename(&backup_dir, target_dir) {
            return Err(err).with_context(|| {
                format!(
                    "rollback also failed ({rollback_err}) - installation directory may be lost"
                )
            });
        }

        return Err(err);
    }

    let _ = cleanup_path(&backup_dir);

    Ok(())
}

fn rename_path(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

/// Extracts `zip_path` into `destination_dir`, rejecting entries with invalid paths.
///
/// The extraction target is validated so the archive cannot be unpacked through
/// an existing reparse-point ancestor.
pub fn extract_zip_archive(zip_path: &Path, destination_dir: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("failed to open zip archive {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("failed to open zip archive")?;
    const ZIP_COPY_BUFFER_SIZE: usize = 256 * 1024;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("failed to read zip entry")?;
        let enclosed_name = entry
            .enclosed_name()
            .ok_or_else(|| anyhow::anyhow!("zip entry contains an invalid path"))?;
        let outpath = destination_dir.join(enclosed_name);

        validate_extraction_target(&outpath)?;

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
        let mut buffer = [0u8; ZIP_COPY_BUFFER_SIZE];

        loop {
            let bytes_read = entry
                .read(&mut buffer)
                .with_context(|| format!("failed to read zip entry {}", outpath.display()))?;
            if bytes_read == 0 {
                break;
            }

            outfile
                .write_all(&buffer[..bytes_read])
                .with_context(|| format!("failed to extract {}", outpath.display()))?;
        }
    }

    Ok(())
}

pub(crate) fn backup_directory_path(target_dir: &Path) -> PathBuf {
    let parent = target_dir.parent().unwrap_or(target_dir);
    let name = target_dir
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_default();

    parent.join(format!("{name}.old"))
}

fn validate_extraction_target(path: &Path) -> Result<()> {
    let mut current = Some(path);

    while let Some(candidate) = current {
        match fs::symlink_metadata(candidate) {
            Ok(metadata) => {
                if is_reparse_point(&metadata) {
                    return Err(anyhow::anyhow!(
                        "refusing to extract through reparse point {}",
                        candidate.display()
                    ));
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to inspect {}", candidate.display()));
            }
        }

        current = candidate.parent();
    }

    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

fn deferred_delete_path(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy();
    let suffix = DEFERRED_DELETE_SUFFIX.fetch_add(1, Ordering::Relaxed);

    Some(path.with_file_name(format!("{file_name}.deleted.{}.{}", process::id(), suffix)))
}
