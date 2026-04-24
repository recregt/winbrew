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

#[cfg(test)]
mod tests {
    use super::cleanup_orphan_install_dirs;
    use anyhow::Result;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn cleanup_orphan_install_dirs_removes_existing_paths_and_ignores_missing_ones() -> Result<()> {
        let root = tempdir().expect("temp dir");
        let existing_orphan = root.path().join("Contoso.Orphan");
        let missing_orphan = root.path().join("Contoso.Missing");

        fs::create_dir_all(&existing_orphan).expect("create orphan directory");
        fs::write(existing_orphan.join("tool.exe"), b"binary").expect("write orphan file");

        let removed = cleanup_orphan_install_dirs(&[existing_orphan.clone(), missing_orphan])?;

        assert_eq!(removed, 1);
        assert!(!existing_orphan.exists());

        Ok(())
    }
}
