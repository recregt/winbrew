use anyhow::Result;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub fn remove(
    _package_name: &str,
    install_dir: &Path,
    _msix_package_full_name: Option<&str>,
) -> Result<()> {
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
        let install_dir = temp_root.path().join("packages").join("Contoso.Zip");

        fs::create_dir_all(&install_dir).expect("create install dir");
        fs::create_dir_all(install_dir.join("bin")).expect("create bin dir");
        fs::write(install_dir.join("bin").join("tool.exe"), b"binary").expect("write file");

        remove("Contoso.Zip", &install_dir, None).expect("remove directory");

        assert!(!install_dir.exists());
    }

    #[test]
    fn remove_allows_missing_directory() {
        let temp_root = tempdir().expect("temp root");
        let install_dir = temp_root.path().join("packages").join("Contoso.Missing");

        remove("Contoso.Missing", &install_dir, None).expect("missing directory should be ignored");
    }
}
