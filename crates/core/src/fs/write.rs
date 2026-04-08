use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process;

use super::cleanup::cleanup_path;

/// Writes `contents` to `path` through `temp_path` and publishes the result atomically.
///
/// The temp file is synced before rename, so callers either see the
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
///
/// This is useful when the caller wants a predictable temporary name per
/// process and does not need to manage the temp file path directly.
pub fn atomic_write_temp(path: &Path, contents: &str) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::atomic_write;
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
}
