use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use winbrew_core::fs::{cleanup_path, replace_directory};

use winbrew_models::install::engine::EngineInstallReceipt;
use winbrew_models::install::engine::EngineKind;

pub fn install(
    download_path: &Path,
    install_dir: &Path,
    _package_name: &str,
) -> Result<EngineInstallReceipt> {
    let stage_dir = staging_dir_for(install_dir);

    cleanup_path(&stage_dir)?;
    fs::create_dir_all(&stage_dir)?;

    let file_name = download_path
        .file_name()
        .context("download path has no file name")?;
    let target_path = stage_dir.join(file_name);

    match fs::rename(download_path, &target_path) {
        Ok(()) => {}
        Err(_) => {
            fs::copy(download_path, &target_path).with_context(|| {
                format!("failed to copy installer to {}", target_path.display())
            })?;
        }
    }

    replace_directory(&stage_dir, install_dir)?;

    Ok(EngineInstallReceipt::new(
        EngineKind::Portable,
        install_dir.to_string_lossy().into_owned(),
        None,
    ))
}

fn staging_dir_for(install_dir: &Path) -> PathBuf {
    install_dir.parent().unwrap_or(install_dir).join("staging")
}

#[cfg(test)]
mod tests {
    use super::install;
    use std::fs;
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn install_copies_non_zip_installer_into_place() {
        let temp_root = tempdir().expect("temp root");
        let download_path = temp_root.path().join("tool.exe");
        let install_dir = temp_root.path().join("packages").join("Contoso.Portable");

        fs::write(&download_path, b"portable-binary").expect("write download");

        install(&download_path, &install_dir, "Contoso.Portable").expect("portable install");

        let installed_file = install_dir.join("tool.exe");
        let mut contents = String::default();
        fs::File::open(&installed_file)
            .expect("installed file")
            .read_to_string(&mut contents)
            .expect("read installed file");

        assert_eq!(contents, "portable-binary");
    }
}
