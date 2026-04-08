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

/// Replaces `source_dir` with `target_dir`, copying across volumes when rename
/// is not available and rolling back the backup on failure.
pub fn replace_directory(source_dir: &Path, target_dir: &Path) -> Result<()> {
    replace_directory_with_rename(source_dir, target_dir, rename_path)
}

fn replace_directory_with_rename<R>(source_dir: &Path, target_dir: &Path, rename: R) -> Result<()>
where
    R: Fn(&Path, &Path) -> std::io::Result<()>,
{
    if !target_dir.exists() {
        return match rename(source_dir, target_dir) {
            Ok(()) => Ok(()),
            Err(err) if is_cross_device_error(&err) => {
                copy_dir_all(source_dir, target_dir).with_context(|| {
                    format!(
                        "failed to copy staged installation across volumes: {} -> {}",
                        source_dir.display(),
                        target_dir.display()
                    )
                })?;

                let _ = cleanup_path(source_dir);

                Ok(())
            }
            Err(err) => Err(err).with_context(|| {
                format!(
                    "failed to move staged installation into place: {} -> {}",
                    source_dir.display(),
                    target_dir.display()
                )
            }),
        };
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

    match rename(source_dir, target_dir) {
        Ok(()) => {
            let _ = cleanup_path(&backup_dir);
            Ok(())
        }
        Err(err) if is_cross_device_error(&err) => {
            if let Err(copy_err) = copy_dir_all(source_dir, target_dir) {
                let _ = cleanup_path(target_dir);

                if let Err(rollback_err) = rename(&backup_dir, target_dir) {
                    return Err(copy_err).with_context(|| {
                        format!(
                            "rollback also failed ({rollback_err}) - installation directory may be lost"
                        )
                    });
                }

                return Err(copy_err).with_context(|| {
                    format!(
                        "failed to copy staged installation across volumes: {} -> {}",
                        source_dir.display(),
                        target_dir.display()
                    )
                });
            }

            let _ = cleanup_path(source_dir);
            let _ = cleanup_path(&backup_dir);

            Ok(())
        }
        Err(err) => {
            if let Err(rollback_err) = rename(&backup_dir, target_dir) {
                return Err(err).with_context(|| {
                    format!(
                        "rollback also failed ({rollback_err}) - installation directory may be lost"
                    )
                });
            }

            Err(err).with_context(|| {
                format!(
                    "failed to move staged installation into place: {} -> {}",
                    source_dir.display(),
                    target_dir.display()
                )
            })
        }
    }
}

fn rename_path(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

fn copy_dir_all(source_dir: &Path, target_dir: &Path) -> Result<()> {
    fs::create_dir_all(target_dir).with_context(|| {
        format!(
            "failed to create destination directory {}",
            target_dir.display()
        )
    })?;

    for entry in fs::read_dir(source_dir)
        .with_context(|| format!("failed to read source directory {}", source_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source_dir.display()))?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect entry {}", source_path.display()))?;

        if file_type.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path)
                .with_context(|| format!("failed to copy file {}", source_path.display()))?;
        } else if file_type.is_symlink() {
            return Err(anyhow::anyhow!(
                "refusing to copy symlink {}",
                source_path.display()
            ));
        } else {
            return Err(anyhow::anyhow!(
                "unsupported entry type {}",
                source_path.display()
            ));
        }
    }

    Ok(())
}

fn is_cross_device_error(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(17) | Some(18))
}

/// Extracts `zip_path` into `destination_dir`, rejecting entries with invalid paths.
///
/// The extraction target is validated so the archive cannot be unpacked through
/// an existing reparse-point ancestor, and symlink entries are refused.
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

        if entry.is_symlink() {
            return Err(anyhow::anyhow!(
                "refusing to extract symlink entry {}",
                outpath.display()
            ));
        }

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
        let mut buffer = vec![0u8; ZIP_COPY_BUFFER_SIZE];

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

#[cfg(test)]
mod tests {
    use super::{extract_zip_archive, replace_directory_with_rename};
    use std::fs;
    use std::io;
    use tempfile::tempdir;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn create_symlink_archive(path: &std::path::Path, link_name: &str, target: &str) {
        let file = fs::File::create(path).expect("create zip file");
        let mut writer = ZipWriter::new(file);
        writer
            .add_symlink(link_name, target, SimpleFileOptions::default())
            .expect("add zip symlink");
        writer.finish().expect("finish zip file");
    }

    #[test]
    fn replace_directory_copies_across_volumes_when_rename_fails() {
        let temp_dir = tempdir().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&source_dir).expect("source dir");
        fs::write(source_dir.join("payload.txt"), b"copied payload").expect("source file");

        let result = replace_directory_with_rename(&source_dir, &target_dir, |from, to| {
            if from == source_dir.as_path() && to == target_dir.as_path() {
                Err(io::Error::from_raw_os_error(18))
            } else {
                fs::rename(from, to)
            }
        });

        result.expect("cross-volume replacement");
        assert_eq!(
            fs::read_to_string(target_dir.join("payload.txt")).expect("copied payload"),
            "copied payload"
        );
        assert!(!source_dir.exists());
    }

    #[test]
    fn extract_zip_archive_rejects_symlink_entries() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_symlink_archive(&zip_path, "bin/tool.exe", "target.exe");

        let error = extract_zip_archive(&zip_path, &destination_dir)
            .expect_err("expected symlink rejection");

        assert!(
            error
                .to_string()
                .contains("refusing to extract symlink entry")
        );
        assert!(!destination_dir.join("bin").exists());
    }
}
