use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::cleanup::cleanup_path;

/// Replaces `source_dir` with `target_dir`, copying across volumes when rename
/// is not available and rolling back the backup on failure.
///
/// On Windows, cross-volume rename failures fall back to copy + cleanup instead
/// of failing the install outright.
pub fn replace_directory(source_dir: &Path, target_dir: &Path) -> Result<()> {
    replace_directory_with_rename(source_dir, target_dir, rename_path)
}

/// Returns the sibling `.old` backup path used during directory replacement.
pub fn backup_path_for(target_dir: &Path) -> PathBuf {
    let parent = target_dir.parent().unwrap_or(target_dir);
    let name = target_dir
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_default();

    parent.join(format!("{name}.old"))
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

    let backup_dir = backup_path_for(target_dir);
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

#[cfg(test)]
mod tests {
    use super::{backup_path_for, replace_directory_with_rename};
    use std::fs;
    use std::io::{self, ErrorKind};
    use tempfile::tempdir;

    #[test]
    fn backup_path_for_appends_old_suffix_next_to_target() {
        let path = std::path::Path::new(r"C:\pkg\tool.exe");
        assert_eq!(
            backup_path_for(path),
            std::path::Path::new(r"C:\pkg\tool.exe.old")
        );
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
    fn replace_directory_restores_backup_on_failure() {
        let temp_dir = tempdir().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        let backup_dir = backup_path_for(&target_dir);

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
        let backup_dir = backup_path_for(&target_dir);

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
