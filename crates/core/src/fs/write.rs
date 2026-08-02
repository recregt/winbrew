use super::FsError;
use super::cleanup::cleanup_path;
use super::move_or_copy::backup_path_for;
use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::process;

type BoxedResult<T> = std::result::Result<T, Box<FsError>>;

/// Writes `contents` to `path` through `temp_path` and publishes the result atomically.
///
/// The temp file is synced before rename, so callers either see the
/// old file or the fully-written new file. The temp file is removed on failure.
pub fn atomic_write(path: &Path, temp_path: &Path, contents: &[u8]) -> BoxedResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| Box::new(FsError::create_directory(parent, err)))?;
    }

    if let Err(err) = write_temp_contents(temp_path, contents) {
        let _ = fs::remove_file(temp_path);
        return Err(err);
    }

    if let Err(err) = finalize_temp_file(temp_path, path) {
        // atomic_write owns the temp path, so it cleans up here even though
        // finalize_temp_file leaves cleanup to direct callers on failure.
        let _ = fs::remove_file(temp_path);
        return Err(err);
    }

    Ok(())
}

/// Writes `contents` to a PID-scoped TOML temp file and atomically publishes it.
///
/// This is useful when the caller wants a predictable temporary name per
/// process and does not need to manage the temp file path directly.
pub fn atomic_write_toml_temp(path: &Path, contents: &str) -> BoxedResult<()> {
    let temp_path = path.with_extension(format!("toml.{}.tmp", process::id()));
    atomic_write(path, &temp_path, contents.as_bytes())
}

/// Replaces `final_path` with `temp_path`, moving any existing conflicting
/// target aside first.
///
/// If this helper is called directly and the rename fails, the caller remains
/// responsible for cleaning up `temp_path`.
pub fn finalize_temp_file(temp_path: &Path, final_path: &Path) -> BoxedResult<()> {
    match fs::rename(temp_path, final_path) {
        Ok(()) => Ok(()),
        Err(err) if is_target_conflict_error(&err) => {
            finalize_with_conflicting_target(temp_path, final_path)
        }
        Err(err) => Err(Box::new(FsError::finalize_file(temp_path, final_path, err))),
    }
}

/// Handles a `final_path` that `fs::rename` cannot overwrite directly -- most
/// commonly because it currently exists as a directory, which no platform's
/// rename allows replacing with a file in one step.
///
/// The conflicting target is moved aside rather than deleted outright, so a
/// failure on the follow-up rename can restore it. Deleting first (the
/// previous approach) broke `atomic_write`'s documented "old file, or
/// fully-written new file, never neither" guarantee: if the rename into
/// place failed after the delete, `final_path` was left permanently missing
/// with the old content already destroyed and nothing to recover it from.
fn finalize_with_conflicting_target(temp_path: &Path, final_path: &Path) -> BoxedResult<()> {
    finalize_with_conflicting_target_using(temp_path, final_path, rename_path)
}

fn rename_path(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

/// Same as [`finalize_with_conflicting_target`], with the rename operation
/// injectable so tests can force the post-backup rename to fail and assert
/// the restore behavior without needing to fabricate a real OS-level race.
fn finalize_with_conflicting_target_using<R>(
    temp_path: &Path,
    final_path: &Path,
    rename: R,
) -> BoxedResult<()>
where
    R: Fn(&Path, &Path) -> std::io::Result<()>,
{
    let backup_path = backup_path_for(final_path);
    cleanup_path(&backup_path)?;

    rename(final_path, &backup_path).map_err(|err| {
        Box::new(FsError::move_aside_before_finalize(
            final_path,
            &backup_path,
            err,
        ))
    })?;

    match rename(temp_path, final_path) {
        Ok(()) => {
            let _ = cleanup_path(&backup_path);
            Ok(())
        }
        Err(err) => {
            if let Err(rollback_err) = rename(&backup_path, final_path) {
                return Err(Box::new(FsError::finalize_rollback_failed(
                    temp_path,
                    final_path,
                    err,
                    rollback_err,
                )));
            }

            Err(Box::new(FsError::finalize_file(temp_path, final_path, err)))
        }
    }
}

fn write_temp_contents(temp_path: &Path, contents: &[u8]) -> BoxedResult<()> {
    let mut file = fs::File::create(temp_path)
        .map_err(|err| Box::new(FsError::create_temp_file(temp_path, err)))?;
    file.write_all(contents)
        .map_err(|err| Box::new(FsError::write_temp_file(temp_path, err)))?;
    file.sync_all()
        .map_err(|err| Box::new(FsError::sync_temp_file(temp_path, err)))?;

    Ok(())
}

fn is_target_conflict_error(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::AlreadyExists | ErrorKind::PermissionDenied | ErrorKind::IsADirectory
    )
}

#[cfg(test)]
mod tests {
    use super::{atomic_write, finalize_temp_file, finalize_with_conflicting_target_using};
    use std::fs;
    use std::io::{self, ErrorKind};
    use tempfile::tempdir;

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
    fn atomic_write_replaces_existing_directory() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("config.toml");
        let temp_path = temp_dir.path().join("config.toml.tmp");

        fs::create_dir(&path).expect("existing final dir");

        atomic_write(&path, &temp_path, b"name=winbrew").expect("atomic write");

        assert_eq!(
            fs::read_to_string(&path).expect("read content"),
            "name=winbrew"
        );
        assert!(!temp_path.exists());
    }

    #[test]
    fn finalize_temp_file_replaces_existing_directory() {
        let temp_dir = tempdir().expect("temp dir");
        let final_path = temp_dir.path().join("config.toml");
        let temp_path = temp_dir.path().join("config.toml.tmp");

        fs::create_dir(&final_path).expect("existing final dir");
        fs::write(&temp_path, b"name=winbrew").expect("write temp file");

        finalize_temp_file(&temp_path, &final_path).expect("finalize temp file");

        assert_eq!(
            fs::read_to_string(&final_path).expect("read content"),
            "name=winbrew"
        );
        assert!(!temp_path.exists());
    }

    /// Locks in the atomic_write guarantee ("old file, or fully-written new
    /// file, never neither") for the specific case where finalizing a
    /// conflicting target fails partway through: the previous approach
    /// deleted the conflicting target before attempting the rename, so a
    /// failure here used to leave `final_path` permanently missing with the
    /// old content already destroyed. It must now be restored instead.
    #[test]
    fn finalize_with_conflicting_target_restores_backup_when_rename_into_place_fails() {
        let temp_dir = tempdir().expect("temp dir");
        let final_path = temp_dir.path().join("config.toml");
        let temp_path = temp_dir.path().join("config.toml.tmp");

        fs::create_dir(&final_path).expect("existing final dir");
        fs::write(&temp_path, b"new content").expect("write temp file");

        let result = finalize_with_conflicting_target_using(&temp_path, &final_path, |from, to| {
            if from == temp_path.as_path() && to == final_path.as_path() {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "simulated finalize failure",
                ))
            } else {
                fs::rename(from, to)
            }
        });

        assert!(result.is_err(), "the simulated failure should propagate");
        assert!(
            final_path.exists(),
            "final_path must never be left missing -- the original target should be restored"
        );
        assert!(
            final_path.is_dir(),
            "the restored target should be the original directory, not a fresh empty one"
        );
        assert!(
            temp_path.exists(),
            "the temp file is untouched on this failure path; the caller (atomic_write) owns cleanup"
        );
    }

    /// When even the restore rename fails, the caller must be told the
    /// original target could not be recovered rather than seeing an
    /// ordinary-looking finalize error that implies the old content survived
    /// at `final_path`.
    #[test]
    fn finalize_with_conflicting_target_reports_rollback_failure_distinctly() {
        let temp_dir = tempdir().expect("temp dir");
        let final_path = temp_dir.path().join("config.toml");
        let temp_path = temp_dir.path().join("config.toml.tmp");
        let backup_path = super::backup_path_for(&final_path);

        fs::create_dir(&final_path).expect("existing final dir");
        fs::write(&temp_path, b"new content").expect("write temp file");

        let result = finalize_with_conflicting_target_using(&temp_path, &final_path, |from, to| {
            if (from == temp_path.as_path() && to == final_path.as_path())
                || (from == backup_path.as_path() && to == final_path.as_path())
            {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "simulated failure",
                ))
            } else {
                fs::rename(from, to)
            }
        });

        let error = result.expect_err("expected a rollback failure");
        assert!(error.to_string().contains("rollback also failed"));
        assert!(
            backup_path.exists(),
            "the backup should still hold the original content when rollback itself failed"
        );
    }
}
