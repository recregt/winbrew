use std::fs;
use std::io;

use crate::core::ResolvedPaths;

pub(crate) fn ensure_managed_root_dirs(paths: &ResolvedPaths) -> std::io::Result<()> {
    let db_dir = paths
        .db
        .parent()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "database path is missing its parent directory",
            )
        })?
        .to_path_buf();

    for dir in [
        &paths.data,
        &paths.pkgdb,
        &db_dir,
        &paths.logs,
        &paths.cache,
    ] {
        fs::create_dir_all(dir)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ensure_managed_root_dirs;
    use crate::core::resolved_paths;
    use tempfile::tempdir;

    #[test]
    fn creates_shared_managed_root_directories_only() {
        let root = tempdir().expect("temp dir");
        let paths = resolved_paths(
            root.path(),
            "${root}\\packages",
            "${root}\\data",
            "${root}\\data\\logs",
            "${root}\\data\\cache",
        );

        ensure_managed_root_dirs(&paths).expect("bootstrap dirs");

        assert!(paths.data.exists());
        assert!(paths.pkgdb.exists());
        assert!(paths.db.parent().expect("db parent").exists());
        assert!(paths.logs.exists());
        assert!(paths.cache.exists());
        assert!(!paths.packages.exists());
    }
}
