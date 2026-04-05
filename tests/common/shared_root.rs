use tempfile::TempDir;

pub fn test_root() -> TempDir {
    tempfile::tempdir().expect("failed to create test root")
}
