use super::FsError;
use super::cleanup::cleanup_path;
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

/// Replaces `final_path` with `temp_path`, removing any existing target first.
pub fn finalize_temp_file(temp_path: &Path, final_path: &Path) -> BoxedResult<()> {
    match fs::rename(temp_path, final_path) {
        Ok(()) => Ok(()),
        Err(err) if is_target_conflict_error(&err) => {
            cleanup_path(final_path)?;

            fs::rename(temp_path, final_path)
                .map_err(|err| Box::new(FsError::finalize_file(temp_path, final_path, err)))
        }
        Err(err) => Err(Box::new(FsError::finalize_file(temp_path, final_path, err))),
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
    use super::{atomic_write, finalize_temp_file};
    use std::fs;
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
}
