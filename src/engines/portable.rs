use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::core::network::installer_filename;

use crate::core::fs::{cleanup_path, extract_zip_archive, replace_directory};

pub fn install(download_path: &Path, install_dir: &Path, installer_url: &str) -> Result<()> {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");

    cleanup_path(&stage_dir)?;
    fs::create_dir_all(&stage_dir)?;

    if is_zip_installer(installer_url) {
        extract_zip_archive(download_path, &stage_dir)?;
    } else {
        let file_name = installer_filename(installer_url);
        let target_path = stage_dir.join(file_name);
        fs::copy(download_path, &target_path)?;
    }

    replace_directory(&stage_dir, install_dir)?;

    Ok(())
}

fn is_zip_installer(url: &str) -> bool {
    url.rsplit_once('.')
        .map(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
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

        install(
            &download_path,
            &install_dir,
            "https://example.invalid/downloads/tool.exe",
        )
        .expect("portable install");

        let installed_file = install_dir.join("tool.exe");
        let mut contents = String::new();
        fs::File::open(&installed_file)
            .expect("installed file")
            .read_to_string(&mut contents)
            .expect("read installed file");

        assert_eq!(contents, "portable-binary");
    }
}
