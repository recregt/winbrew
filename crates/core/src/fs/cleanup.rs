use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(windows)]
use winbrew_windows::inspect_path as winfs_inspect_path;

static DEFERRED_DELETE_SUFFIX: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy)]
pub(super) struct PathInfo {
    pub(super) is_directory: bool,
    pub(super) is_reparse_point: bool,
    pub(super) hard_link_count: u32,
}

pub(super) fn inspect_path(path: &Path) -> std::io::Result<PathInfo> {
    #[cfg(windows)]
    {
        let info = winfs_inspect_path(path)?;
        Ok(PathInfo {
            is_directory: info.is_directory,
            is_reparse_point: info.is_reparse_point,
            hard_link_count: info.hard_link_count,
        })
    }

    #[cfg(not(windows))]
    {
        let metadata = fs::symlink_metadata(path)?;
        Ok(PathInfo {
            is_directory: metadata.is_dir(),
            is_reparse_point: false,
            hard_link_count: 1,
        })
    }
}

/// Removes `path` if it exists.
///
/// If immediate deletion fails and the path has a file name, the item is moved
/// aside to a deferred-delete path so cleanup can continue later. On Windows,
/// directory reparse points are removed without recursively walking their target.
pub fn cleanup_path(path: &Path) -> Result<()> {
    let info = match inspect_path(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };

    let removal_result = if info.is_reparse_point {
        fs::remove_dir(path).or_else(|_| fs::remove_file(path))
    } else if info.is_directory {
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

fn deferred_delete_path(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy();
    let suffix = DEFERRED_DELETE_SUFFIX.fetch_add(1, Ordering::Relaxed);

    Some(path.with_file_name(format!("{file_name}.deleted.{}.{}", process::id(), suffix)))
}

#[cfg(test)]
mod tests {
    use super::cleanup_path;
    use tempfile::tempdir;

    #[test]
    fn cleanup_path_is_noop_when_path_missing() {
        let temp_dir = tempdir().expect("temp dir");
        let missing = temp_dir.path().join("missing");

        assert!(cleanup_path(&missing).is_ok());
        assert!(!missing.exists());
    }
}
