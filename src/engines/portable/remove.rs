use anyhow::Result;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub fn remove(install_dir: &Path) -> Result<()> {
    match fs::remove_dir_all(install_dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::remove;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn remove_deletes_existing_directory() {
        let temp_root = tempdir().expect("temp root");
        let install_dir = temp_root.path().join("packages").join("Contoso.Portable");

        fs::create_dir_all(&install_dir).expect("create install dir");
        fs::write(install_dir.join("tool.exe"), b"binary").expect("write file");

        remove(&install_dir).expect("remove directory");

        assert!(!install_dir.exists());
    }

    #[test]
    fn remove_allows_missing_directory() {
        let temp_root = tempdir().expect("temp root");
        let install_dir = temp_root.path().join("packages").join("Contoso.Missing");

        remove(&install_dir).expect("missing directory should be ignored");
    }
}
