use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::fs::{cleanup_path, extract_zip_archive, replace_directory};
use crate::network::installer_filename;

pub fn install(download_path: &Path, install_dir: &Path, installer_url: &str) -> Result<()> {
    let stage_dir = staging_dir_for(install_dir);

    cleanup_path(&stage_dir)?;
    fs::create_dir_all(&stage_dir)?;

    if is_zip_installer(installer_url) {
        extract_zip_archive(download_path, &stage_dir)?;
    } else {
        let file_name = installer_filename(installer_url);
        let target_path = stage_dir.join(file_name);

        match fs::rename(download_path, &target_path) {
            Ok(()) => {}
            Err(_) => {
                fs::copy(download_path, &target_path).with_context(|| {
                    format!("failed to copy installer to {}", target_path.display())
                })?;
            }
        }
    }

    replace_directory(&stage_dir, install_dir)?;

    Ok(())
}

fn staging_dir_for(install_dir: &Path) -> PathBuf {
    install_dir.parent().unwrap_or(install_dir).join("staging")
}

fn is_zip_installer(url: &str) -> bool {
    let clean_url = url.split('?').next().unwrap_or(url);
    let clean_url = clean_url.split('#').next().unwrap_or(clean_url);

    clean_url
        .rsplit_once('.')
        .map(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::install;
    use std::fs;
    use std::io::{Read, Write};
    use tempfile::tempdir;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn create_zip_archive(path: &std::path::Path, file_name: &str, contents: &[u8]) {
        let file = fs::File::create(path).expect("create zip file");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file(file_name, SimpleFileOptions::default())
            .expect("start zip entry");
        writer.write_all(contents).expect("write zip contents");
        writer.finish().expect("finish zip file");
    }

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

    #[test]
    fn install_extracts_zip_installer_with_query_string() {
        let temp_root = tempdir().expect("temp root");
        let download_path = temp_root.path().join("download.zip");
        let install_dir = temp_root
            .path()
            .join("packages")
            .join("Contoso.PortableZip");

        create_zip_archive(&download_path, "bin/tool.exe", b"zip-binary");

        install(
            &download_path,
            &install_dir,
            "https://example.invalid/downloads/tool.zip?token=123#fragment",
        )
        .expect("portable zip install");

        let installed_file = install_dir.join("bin").join("tool.exe");
        let mut contents = String::new();
        fs::File::open(&installed_file)
            .expect("installed file")
            .read_to_string(&mut contents)
            .expect("read installed file");

        assert_eq!(contents, "zip-binary");
    }
}
