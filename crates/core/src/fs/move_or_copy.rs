use super::FsError;
use super::cleanup::{cleanup_path, inspect_path as inspect_cleanup_path};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

type BoxedResult<T> = std::result::Result<T, Box<FsError>>;

/// Replaces `source_dir` with `target_dir`, copying across volumes when rename
/// is not available and rolling back the backup on failure.
///
/// On Windows, cross-volume rename failures fall back to copy + cleanup instead
/// of failing the install outright.
pub fn replace_directory(source_dir: &Path, target_dir: &Path) -> BoxedResult<()> {
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

fn replace_directory_with_rename<R>(
    source_dir: &Path,
    target_dir: &Path,
    rename: R,
) -> BoxedResult<()>
where
    R: Fn(&Path, &Path) -> std::io::Result<()>,
{
    let backup_dir = backup_path_for(target_dir);
    cleanup_path(&backup_dir)?;

    match rename(source_dir, target_dir) {
        Ok(()) => Ok(()),
        Err(err) if is_cross_device_error(&err) => match rename(target_dir, &backup_dir) {
            Ok(()) => finish_replacement_after_backup(source_dir, target_dir, &backup_dir, rename),
            Err(rename_err) if rename_err.kind() == ErrorKind::NotFound => {
                // Cross-device fallback copies the staged directory tree after the
                // original target has been moved aside.
                copy_dir_all(source_dir, target_dir).map_err(|copy_err| {
                    Box::new(FsError::copy_across_volumes(
                        source_dir, target_dir, copy_err,
                    ))
                })?;

                let _ = cleanup_path(source_dir);

                Ok(())
            }
            Err(rename_err) => Err(Box::new(FsError::move_aside(
                target_dir,
                &backup_dir,
                rename_err,
            ))),
        },
        Err(err) if is_target_conflict_error(&err) => match rename(target_dir, &backup_dir) {
            Ok(()) => finish_replacement_after_backup(source_dir, target_dir, &backup_dir, rename),
            Err(rename_err) if rename_err.kind() == ErrorKind::NotFound => Err(Box::new(
                FsError::move_into_place(source_dir, target_dir, err),
            )),
            Err(rename_err) => Err(Box::new(FsError::move_aside(
                target_dir,
                &backup_dir,
                rename_err,
            ))),
        },
        Err(err) => Err(Box::new(FsError::move_into_place(
            source_dir, target_dir, err,
        ))),
    }
}

fn finish_replacement_after_backup<R>(
    source_dir: &Path,
    target_dir: &Path,
    backup_dir: &Path,
    rename: R,
) -> BoxedResult<()>
where
    R: Fn(&Path, &Path) -> std::io::Result<()>,
{
    match rename(source_dir, target_dir) {
        Ok(()) => {
            let _ = cleanup_path(backup_dir);
            Ok(())
        }
        Err(err) if is_cross_device_error(&err) => {
            if let Err(copy_err) = copy_dir_all(source_dir, target_dir) {
                let _ = cleanup_path(target_dir);

                if let Err(rollback_err) = rename(backup_dir, target_dir) {
                    return Err(Box::new(FsError::rollback_failed(
                        "failed to copy staged installation across volumes",
                        source_dir,
                        target_dir,
                        copy_err,
                        rollback_err,
                    )));
                }

                let _ = cleanup_path(source_dir);

                return Err(Box::new(FsError::copy_across_volumes(
                    source_dir, target_dir, copy_err,
                )));
            }

            let _ = cleanup_path(source_dir);
            let _ = cleanup_path(backup_dir);

            Ok(())
        }
        Err(err) => {
            if let Err(rollback_err) = rename(backup_dir, target_dir) {
                return Err(Box::new(FsError::rollback_failed(
                    "failed to move staged installation into place",
                    source_dir,
                    target_dir,
                    err,
                    rollback_err,
                )));
            }

            Err(Box::new(FsError::move_into_place(
                source_dir, target_dir, err,
            )))
        }
    }
}

fn rename_path(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

fn copy_dir_all(source_dir: &Path, target_dir: &Path) -> BoxedResult<()> {
    fs::create_dir_all(target_dir)
        .map_err(|err| Box::new(FsError::create_directory(target_dir, err)))?;

    for entry in fs::read_dir(source_dir)
        .map_err(|err| Box::new(FsError::read_directory(source_dir, err)))?
    {
        let entry =
            entry.map_err(|err| Box::new(FsError::read_directory_entry(source_dir, err)))?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());

        // `file_type()` tells us file vs directory, but the path inspection is
        // still needed to reject Windows reparse points explicitly.
        let path_info = inspect_cleanup_path(&source_path)
            .map_err(|err| Box::new(FsError::inspect(&source_path, err)))?;
        let file_type = entry
            .file_type()
            .map_err(|err| Box::new(FsError::inspect(&source_path, err)))?;

        if path_info.is_reparse_point {
            return Err(Box::new(FsError::copy_symlink(&source_path)));
        } else if file_type.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path)
                .map_err(|err| Box::new(FsError::copy_file(&source_path, &target_path, err)))?;
        } else if file_type.is_symlink() {
            return Err(Box::new(FsError::copy_symlink(&source_path)));
        } else {
            return Err(Box::new(FsError::unsupported_entry(&source_path)));
        }
    }

    Ok(())
}

fn is_cross_device_error(err: &std::io::Error) -> bool {
    matches!(err.kind(), ErrorKind::CrossesDevices)
}

#[cfg(windows)]
fn is_target_conflict_error(err: &io::Error) -> bool {
    // Windows may report a target that already exists or is otherwise busy as
    // PermissionDenied, so treat both as a backup-and-retry conflict.
    matches!(
        err.kind(),
        ErrorKind::AlreadyExists | ErrorKind::PermissionDenied
    )
}

#[cfg(not(windows))]
fn is_target_conflict_error(err: &io::Error) -> bool {
    matches!(err.kind(), ErrorKind::AlreadyExists)
}

#[cfg(test)]
mod tests {
    use super::{backup_path_for, replace_directory_with_rename};
    use std::fs;
    use std::io::{self, ErrorKind};
    use tempfile::tempdir;

    fn cross_device_error() -> io::Error {
        io::Error::new(ErrorKind::CrossesDevices, "simulated cross-device error")
    }

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
                Err(cross_device_error())
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
