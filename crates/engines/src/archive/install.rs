use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::core::ArchiveKind;
use crate::core::fs::{cleanup_path, extract_archive, replace_directory};

use crate::models::install::engine::EngineInstallReceipt;
use crate::models::install::engine::EngineKind;

use crate::payload::{DetectedArtifactKind, archive_kind_for_url, probe_downloaded_artifact_kind};

/// Extract an archive installer into the target install directory.
///
/// This preserves the packaged directory tree as-is and does not try to
/// discover or promote a primary binary.
pub(crate) fn install(
    download_path: &Path,
    install_dir: &Path,
    installer_url: &str,
) -> Result<EngineInstallReceipt> {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");
    let archive_kind = resolve_archive_kind(download_path, installer_url);

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

/// Determine which archive format the downloaded payload actually is.
///
/// Prefers the content-probed kind (the same signature check that routed
/// this installer to the archive engine in the first place) over guessing
/// from the URL's file extension. A URL with no recognizable extension --
/// common for CDN redirect links or self-extracting downloads -- would
/// otherwise always fall back to `Zip` here even when routing upstream
/// correctly identified the payload as, say, a 7z or tar archive from its
/// actual bytes: extraction would then fail trying to parse non-ZIP bytes
/// as a ZIP file despite having been routed to the right engine.
fn resolve_archive_kind(download_path: &Path, installer_url: &str) -> ArchiveKind {
    match probe_downloaded_artifact_kind(download_path) {
        Ok(Some(DetectedArtifactKind::Archive(kind))) => kind,
        _ => archive_kind_for_url(installer_url).unwrap_or(ArchiveKind::Zip),
    }
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

    fn create_zip_archive_with_tree(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("create zip file");
        let mut writer = ZipWriter::new(file);

        for (file_name, contents) in entries {
            writer
                .start_file(file_name, SimpleFileOptions::default())
                .expect("start zip entry");
            writer.write_all(contents).expect("write zip contents");
        }

        writer.finish().expect("finish zip file");
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
    fn install_preserves_nested_archive_trees_without_promoting_a_binary() {
        let temp_root = tempdir().expect("temp root");
        let download_path = temp_root.path().join("download.zip");
        let install_dir = temp_root.path().join("packages").join("Contoso.Tree");

        create_zip_archive_with_tree(
            &download_path,
            &[
                ("README.md", b"tree readme"),
                ("docs/notes.txt", b"notes"),
                ("bin/tool.exe", b"tree-binary"),
            ],
        );

        install(
            &download_path,
            &install_dir,
            "https://example.invalid/download.zip",
        )
        .expect("tree archive install");

        let readme = install_dir.join("README.md");
        let notes = install_dir.join("docs").join("notes.txt");
        let binary = install_dir.join("bin").join("tool.exe");

        assert!(readme.exists(), "README should stay at the archive root");
        assert!(notes.exists(), "nested docs should be preserved");
        assert!(binary.exists(), "binary should remain in its original path");

        let mut readme_contents = String::default();
        fs::File::open(&readme)
            .expect("readme file")
            .read_to_string(&mut readme_contents)
            .expect("read readme");

        let mut binary_contents = String::default();
        fs::File::open(&binary)
            .expect("binary file")
            .read_to_string(&mut binary_contents)
            .expect("read binary");

        assert_eq!(readme_contents, "tree readme");
        assert_eq!(binary_contents, "tree-binary");
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

    /// Locks in the fix for archive kind detection: a CDN-style URL with no
    /// recognizable extension used to always fall back to `Zip` here, even
    /// though routing upstream (registry::probe_installer_from_download)
    /// already correctly identified the payload as an archive from its
    /// actual bytes. Extraction would then fail trying to parse tar.gz bytes
    /// as a ZIP file. The downloaded file's own filename intentionally has
    /// no `.tar.gz`/`.tgz` suffix for this test, and the installer URL has
    /// no extension at all -- content probing is the only thing that can get
    /// this right.
    #[test]
    fn install_extracts_gzip_compressed_tar_with_no_recognizable_extension() {
        let temp_root = tempdir().expect("temp root");
        let download_path = temp_root.path().join("download");
        let install_dir = temp_root.path().join("packages").join("Contoso.NoExt");

        create_tar_gz_archive(&download_path, "bin/tool.exe", b"tar-binary");

        install(
            &download_path,
            &install_dir,
            "https://example.invalid/download?id=12345",
        )
        .expect("extensionless tar.gz install");

        let installed_file = install_dir.join("bin").join("tool.exe");
        let mut contents = String::default();
        fs::File::open(&installed_file)
            .expect("installed file")
            .read_to_string(&mut contents)
            .expect("read installed file");

        assert_eq!(contents, "tar-binary");
    }
}
