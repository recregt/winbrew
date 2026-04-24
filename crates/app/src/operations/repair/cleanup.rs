use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

pub fn cleanup_orphan_install_dirs(orphan_paths: &[PathBuf]) -> Result<usize> {
    let mut removed = 0usize;

    for orphan_path in orphan_paths {
        match fs::remove_dir_all(orphan_path) {
            Ok(()) => {
                removed += 1;
            }
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to remove orphan install directory at {}",
                        orphan_path.display()
                    )
                });
            }
        }
    }

    Ok(removed)
}
