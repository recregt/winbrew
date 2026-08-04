use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

/// Outcome of an orphan install-directory cleanup batch.
///
/// Every path in the batch is attempted; a failure on one path does not stop
/// the rest from being cleaned up, and the caller can see exactly which
/// paths succeeded and which failed instead of only learning "something in
/// this batch failed."
#[derive(Debug, Default)]
pub struct OrphanCleanupSummary {
    pub removed: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, anyhow::Error)>,
}

pub fn cleanup_orphan_install_dirs(orphan_paths: &[PathBuf]) -> OrphanCleanupSummary {
    let mut summary = OrphanCleanupSummary::default();

    for orphan_path in orphan_paths {
        match fs::remove_dir_all(orphan_path) {
            Ok(()) => summary.removed.push(orphan_path.clone()),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                let err = anyhow::Error::new(err).context(format!(
                    "failed to remove orphan install directory at {}",
                    orphan_path.display()
                ));
                summary.failed.push((orphan_path.clone(), err));
            }
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::cleanup_orphan_install_dirs;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn cleanup_orphan_install_dirs_removes_existing_paths_and_ignores_missing_ones() {
        let root = tempdir().expect("temp dir");
        let existing_orphan = root.path().join("Contoso.Orphan");
        let missing_orphan = root.path().join("Contoso.Missing");

        fs::create_dir_all(&existing_orphan).expect("create orphan directory");
        fs::write(existing_orphan.join("tool.exe"), b"binary").expect("write orphan file");

        let summary = cleanup_orphan_install_dirs(&[existing_orphan.clone(), missing_orphan]);

        assert_eq!(summary.removed, vec![existing_orphan.clone()]);
        assert!(summary.failed.is_empty());
        assert!(!existing_orphan.exists());
    }

    #[test]
    #[cfg(unix)]
    fn cleanup_orphan_install_dirs_reports_partial_progress_on_failure() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempdir().expect("temp dir");
        let removable = root.path().join("Contoso.Removable");
        let blocked_parent = root.path().join("Contoso.Blocked");
        let blocked_child = blocked_parent.join("nested");

        fs::create_dir_all(&removable).expect("create removable orphan");
        fs::create_dir_all(&blocked_child).expect("create blocked orphan");

        // Remove write permission on the parent so removing its child fails,
        // without preventing the *other* orphan from being cleaned up.
        let mut perms = fs::metadata(&blocked_parent)
            .expect("read blocked parent metadata")
            .permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&blocked_parent, perms).expect("restrict blocked parent");

        let summary = cleanup_orphan_install_dirs(&[removable.clone(), blocked_child.clone()]);

        // Restore permissions so the tempdir can be cleaned up.
        let mut perms = fs::metadata(&blocked_parent)
            .expect("read blocked parent metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&blocked_parent, perms).expect("restore blocked parent");

        assert_eq!(summary.removed, vec![removable]);
        assert_eq!(summary.failed.len(), 1);
        assert_eq!(summary.failed[0].0, blocked_child);
    }
}
