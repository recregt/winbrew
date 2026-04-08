//! File-system helpers used by Winbrew's configuration, download, and install flows.
//!
//! This module centralizes temp-file publishing, best-effort cleanup, directory
//! replacement, and zip extraction so the higher-level code can keep the Windows
//! filesystem behavior consistent in one place.

use anyhow::{Context, Result};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

static DEFERRED_DELETE_SUFFIX: AtomicUsize = AtomicUsize::new(0);

/// Removes `path` if it exists.
///
/// If immediate deletion fails and the path has a file name, the item is moved
/// aside to a deferred-delete path so cleanup can continue later. On Windows,
/// directory reparse points are removed without recursively walking their target.
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

/// Writes `contents` to `path` through `temp_path` and publishes the result atomically.
///
/// The temp file is flushed and synced before rename, so callers either see the
/// old file or the fully-written new file. The temp file is removed on failure.
pub fn atomic_write(path: &Path, temp_path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory for {}", path.display()))?;
    }

    let result = (|| -> Result<()> {
        let mut file = fs::File::create(temp_path)
            .with_context(|| format!("failed to create temp file at {}", temp_path.display()))?;
        file.write_all(contents)
            .with_context(|| format!("failed to write temp file at {}", temp_path.display()))?;
        file.flush()
            .with_context(|| format!("failed to flush temp file at {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync temp file at {}", temp_path.display()))?;

        Ok(())
    })();

    if let Err(err) = result {
        let _ = fs::remove_file(temp_path);
        return Err(err);
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

/// Writes `contents` to a PID-scoped TOML temp file and atomically publishes it.
pub fn atomic_write_with_pid_suffix(path: &Path, contents: &str) -> Result<()> {
    let temp_path = path.with_extension(format!("toml.{}.tmp", process::id()));
    atomic_write(path, &temp_path, contents.as_bytes())
}

/// Replaces `final_path` with `temp_path`, removing any existing target first.
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

/// Replaces `target_dir` with `source_dir`.
///
/// The existing target is moved aside to a sibling backup directory first. If
/// the staged move fails, the backup is moved back into place when possible.
/// If rollback also fails, the returned error includes both failures.
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

/// Returns the sibling `.old` backup path used during directory replacement.
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

#[cfg(test)]
mod tests {
    use super::{atomic_write, backup_directory_path, cleanup_path, replace_directory_with_rename};
    use std::fs;
    use std::io::{self, ErrorKind};
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn backup_directory_path_appends_old_suffix_next_to_target() {
        let path = Path::new(r"C:\pkg\tool.exe");
        assert_eq!(
            backup_directory_path(path),
            Path::new(r"C:\pkg\tool.exe.old")
        );
    }

    #[test]
    fn cleanup_path_is_noop_when_path_missing() {
        let temp_dir = tempdir().expect("temp dir");
        let missing = temp_dir.path().join("missing");

        assert!(cleanup_path(&missing).is_ok());
        assert!(!missing.exists());
    }

    #[test]
    fn atomic_write_produces_correct_content() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("config.toml");
        let temp_path = temp_dir.path().join("config.toml.tmp");

        atomic_write(&path, &temp_path, b"name=winbrew").expect("atomic write");

        assert_eq!(
            fs::read_to_string(&path).expect("read content"),
            "name=winbrew"
        );
        assert!(!temp_path.exists());
    }

    #[test]
    fn replace_directory_restores_backup_on_failure() {
        let temp_dir = tempdir().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        let backup_dir = backup_directory_path(&target_dir);

        fs::create_dir_all(&source_dir).expect("source dir");
        fs::create_dir_all(&target_dir).expect("target dir");
        fs::write(source_dir.join("new.txt"), b"new").expect("source file");
        fs::write(target_dir.join("old.txt"), b"old").expect("target file");

        let result = replace_directory_with_rename(&source_dir, &target_dir, |from, to| {
            if from == source_dir.as_path() && to == target_dir.as_path() {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "simulated failure",
                ))
            } else {
                fs::rename(from, to)
            }
        });

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(target_dir.join("old.txt")).expect("restored"),
            "old"
        );
        assert_eq!(
            fs::read_to_string(source_dir.join("new.txt")).expect("source kept"),
            "new"
        );
        assert!(!backup_dir.exists());
    }

    #[test]
    fn replace_directory_reports_rollback_failure() {
        let temp_dir = tempdir().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        let backup_dir = backup_directory_path(&target_dir);

        fs::create_dir_all(&source_dir).expect("source dir");
        fs::create_dir_all(&target_dir).expect("target dir");

        let result = replace_directory_with_rename(&source_dir, &target_dir, |from, to| {
            if (from == source_dir.as_path() && to == target_dir.as_path())
                || (from == backup_dir.as_path() && to == target_dir.as_path())
            {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "simulated failure",
                ))
            } else {
                fs::rename(from, to)
            }
        });

        let error = result.expect_err("expected rollback failure");
        assert!(error.to_string().contains("rollback also failed"));
        assert!(backup_dir.exists());
        assert!(!target_dir.exists());
    }
}
