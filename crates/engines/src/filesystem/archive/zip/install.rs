use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::core::fs::{cleanup_path, extract_archive, replace_directory};

use crate::models::install::engine::EngineInstallReceipt;
use crate::models::install::engine::EngineKind;

use crate::payload::archive_kind_for_url;

pub fn install(
    download_path: &Path,
    install_dir: &Path,
    installer_url: &str,
) -> Result<EngineInstallReceipt> {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");
    let archive_kind = archive_kind_for_url(installer_url).unwrap_or(crate::core::ArchiveKind::Zip);

    cleanup_path(&stage_dir)?;
    fs::create_dir_all(&stage_dir)?;

    extract_archive(archive_kind, download_path, &stage_dir)?;
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
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::{Read, Write};
    use tar::Builder;
    use tar::Header;
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

    fn create_tar_gz_archive(path: &std::path::Path, file_name: &str, contents: &[u8]) {
        let file = fs::File::create(path).expect("create tar.gz file");
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);
        let mut header = Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder
            .append_data(&mut header, file_name, contents)
            .expect("append tar.gz entry");
        let encoder = builder.into_inner().expect("finish tar builder");
        encoder.finish().expect("finish tar.gz file");
    }

    #[test]
    fn install_extracts_archive_into_install_directory() {
        let temp_root = tempdir().expect("temp root");
        let download_path = temp_root.path().join("download.zip");
        let install_dir = temp_root.path().join("packages").join("Contoso.Zip");

        create_zip_archive(&download_path, "bin/tool.exe", b"zip-binary");

        install(
            &download_path,
            &install_dir,
            "https://example.invalid/download.zip",
        )
        .expect("zip install");

        let installed_file = install_dir.join("bin").join("tool.exe");
        let mut contents = String::default();
        fs::File::open(&installed_file)
            .expect("installed file")
            .read_to_string(&mut contents)
            .expect("read installed file");

        assert_eq!(contents, "zip-binary");
    }

    #[test]
    fn install_extracts_tar_gz_archive_into_install_directory() {
        let temp_root = tempdir().expect("temp root");
        let download_path = temp_root.path().join("download.tar.gz");
        let install_dir = temp_root.path().join("packages").join("Contoso.Tar");

        create_tar_gz_archive(&download_path, "bin/tool.exe", b"tar-binary");

        install(
            &download_path,
            &install_dir,
            "https://example.invalid/download.tar.gz",
        )
        .expect("tar.gz install");

        let installed_file = install_dir.join("bin").join("tool.exe");
        let mut contents = String::default();
        fs::File::open(&installed_file)
            .expect("installed file")
            .read_to_string(&mut contents)
            .expect("read installed file");

        assert_eq!(contents, "tar-binary");
    }
}
