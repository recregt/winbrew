use anyhow::Result;
use std::fs;
use std::io::ErrorKind;

use winbrew_models::install::installed::InstalledPackage;

pub fn remove(package: &InstalledPackage) -> Result<()> {
    match fs::remove_dir_all(&package.install_dir) {
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
    use winbrew_models::install::engine::EngineKind;
    use winbrew_models::install::installed::{InstalledPackage, PackageStatus};
    use winbrew_models::install::installer::InstallerType;

    fn package(name: &str, install_dir: &std::path::Path) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            deployment_kind: InstallerType::Portable.deployment_kind(),
            engine_kind: EngineKind::Portable,
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().into_owned(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn remove_deletes_existing_directory() {
        let temp_root = tempdir().expect("temp root");
        let install_dir = temp_root.path().join("packages").join("Contoso.Portable");

        fs::create_dir_all(&install_dir).expect("create install dir");
        fs::write(install_dir.join("tool.exe"), b"binary").expect("write file");

        remove(&package("Contoso.Portable", &install_dir)).expect("remove directory");

        assert!(!install_dir.exists());
    }

    #[test]
    fn remove_allows_missing_directory() {
        let temp_root = tempdir().expect("temp root");
        let install_dir = temp_root.path().join("packages").join("Contoso.Missing");

        remove(&package("Contoso.Missing", &install_dir))
            .expect("missing directory should be ignored");
    }
}
