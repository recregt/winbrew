use anyhow::Result;
use std::fs;
use std::path::Path;

use winbrew_core::fs::{cleanup_path, extract_zip_archive, replace_directory};

use winbrew_models::install::engine::EngineInstallReceipt;
use winbrew_models::install::engine::EngineKind;

pub fn install(download_path: &Path, install_dir: &Path) -> Result<EngineInstallReceipt> {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");

    cleanup_path(&stage_dir)?;
    fs::create_dir_all(&stage_dir)?;

    extract_zip_archive(download_path, &stage_dir)?;
    replace_directory(&stage_dir, install_dir)?;

    Ok(EngineInstallReceipt::new(
        EngineKind::Zip,
        install_dir.to_string_lossy().into_owned(),
        None,
    ))
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
    fn install_extracts_archive_into_install_directory() {
        let temp_root = tempdir().expect("temp root");
        let download_path = temp_root.path().join("download.zip");
        let install_dir = temp_root.path().join("packages").join("Contoso.Zip");

        create_zip_archive(&download_path, "bin/tool.exe", b"zip-binary");

        install(&download_path, &install_dir).expect("zip install");

        let installed_file = install_dir.join("bin").join("tool.exe");
        let mut contents = String::default();
        fs::File::open(&installed_file)
            .expect("installed file")
            .read_to_string(&mut contents)
            .expect("read installed file");

        assert_eq!(contents, "zip-binary");
    }
}
