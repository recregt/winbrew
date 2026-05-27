use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use crate::fs::{FsError, Result};

use super::super::platform::PlatformAdapter;
use super::{ExtractionContext, ExtractionLimits};

pub(crate) fn extract_zip_archive_with_platform<P: PlatformAdapter>(
    zip_path: &Path,
    destination_dir: &Path,
    limits: ExtractionLimits,
) -> Result<()> {
    let file = fs::File::open(zip_path).map_err(|err| FsError::open_zip_archive(zip_path, err))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|err| FsError::open_zip_archive(zip_path, err))?;
    const ZIP_COPY_BUFFER_SIZE: usize = 256 * 1024;
    let mut extraction = ExtractionContext::<P>::new(limits);
    let mut buffer = vec![0u8; ZIP_COPY_BUFFER_SIZE];

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| FsError::read_zip_entry(zip_path, err))?;
        extract_entry(&mut entry, destination_dir, &mut extraction, &mut buffer)?;
    }

    extraction.commit();
    Ok(())
}

fn extract_entry<P: PlatformAdapter, R: Read>(
    entry: &mut zip::read::ZipFile<'_, R>,
    destination_dir: &Path,
    extraction: &mut ExtractionContext<P>,
    buffer: &mut [u8],
) -> Result<()> {
    let enclosed_name = entry
        .enclosed_name()
        .ok_or_else(FsError::invalid_zip_entry_path)?;

    if entry.is_symlink() {
        return Err(FsError::symlink_entry(
            &destination_dir.join(&enclosed_name),
        ));
    }

    let outpath = destination_dir.join(&enclosed_name);

    extraction.validate_target(&outpath, destination_dir)?;

    extraction.check_limits(&enclosed_name, entry.size(), entry.compressed_size())?;

    if entry.is_dir() {
        extraction.ensure_directory_tree(&outpath)?;
        return Ok(());
    }

    if let Some(parent) = outpath.parent() {
        extraction.ensure_directory_tree(parent)?;
    }

    let mut outfile = P::create_extraction_target_file(&outpath)
        .map_err(|err| FsError::create_extracted_file(&outpath, err))?;
    extraction.record_file(&outpath);

    loop {
        let bytes_read = entry
            .read(buffer)
            .map_err(|err| FsError::read_entry(&outpath, err))?;
        if bytes_read == 0 {
            break;
        }

        outfile
            .write_all(&buffer[..bytes_read])
            .map_err(|err| FsError::write_entry(&outpath, err))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::extract_zip_archive_with_limits;
    use super::*;
    use crate::fs::archive::extract_zip_archive;
    use std::fs;
    use std::io::Write;
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

    fn create_symlink_archive(path: &std::path::Path, link_name: &str, target: &str) {
        let file = fs::File::create(path).expect("create zip file");
        let mut writer = ZipWriter::new(file);
        writer
            .add_symlink(link_name, target, SimpleFileOptions::default())
            .expect("add zip symlink");
        writer.finish().expect("finish zip file");
    }

    fn create_archive_with_entries(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("create zip file");
        let mut writer = ZipWriter::new(file);

        for (name, contents) in entries {
            writer
                .start_file(name, SimpleFileOptions::default())
                .expect("start zip entry");
            writer.write_all(contents).expect("write zip contents");
        }

        writer.finish().expect("finish zip file");
    }

    #[test]
    #[cfg(windows)]
    fn extract_zip_archive_rejects_hardlinked_targets() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let anchor_path = temp_dir.path().join("anchor.txt");
        let target_path = destination_dir.join("payload.txt");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        fs::write(&anchor_path, b"anchor").expect("anchor file");
        fs::hard_link(&anchor_path, &target_path).expect("hard link");
        create_zip_archive(&zip_path, "payload.txt", b"zip payload");

        let error = extract_zip_archive(&zip_path, &destination_dir)
            .expect_err("expected hardlinked target rejection");

        assert!(error.to_string().contains("hardlinked file"));
    }

    #[test]
    fn extract_zip_archive_rejects_symlink_entries() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_symlink_archive(&zip_path, "bin/tool.exe", "target.exe");

        let error = extract_zip_archive(&zip_path, &destination_dir)
            .expect_err("expected symlink rejection");

        assert!(
            error
                .to_string()
                .contains("refusing to extract symlink entry")
        );
        assert!(!destination_dir.join("bin").exists());
    }

    #[test]
    fn extract_zip_archive_cleans_partial_output_on_failure() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        let file = fs::File::create(&zip_path).expect("create zip file");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("bin/ok.txt", SimpleFileOptions::default())
            .expect("start ok entry");
        writer.write_all(b"ok").expect("write ok entry");
        writer
            .add_symlink("bin/bad-link", "target.exe", SimpleFileOptions::default())
            .expect("add symlink entry");
        writer.finish().expect("finish zip file");

        let error = extract_zip_archive(&zip_path, &destination_dir)
            .expect_err("expected cleanup after partial extraction failure");

        assert!(
            error
                .to_string()
                .contains("refusing to extract symlink entry")
        );
        assert!(!destination_dir.exists());
    }

    #[test]
    fn extract_zip_archive_extracts_files_correctly() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("dest dir");
        create_zip_archive(&zip_path, "bin/tool.exe", b"binary content");

        extract_zip_archive(&zip_path, &destination_dir).expect("extraction");

        assert_eq!(
            fs::read(destination_dir.join("bin/tool.exe")).expect("read"),
            b"binary content"
        );
    }

    #[test]
    fn extract_zip_archive_rejects_existing_target_files() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let existing_target = destination_dir.join("bin/tool.exe");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(existing_target.parent().expect("parent dir")).expect("destination dir");
        fs::write(&existing_target, b"existing content").expect("preexisting target");
        create_zip_archive(&zip_path, "bin/tool.exe", b"new content");

        let error = extract_zip_archive(&zip_path, &destination_dir)
            .expect_err("expected overwrite protection");

        assert!(
            error
                .to_string()
                .contains("failed to create extracted file")
        );
        assert_eq!(
            fs::read(&existing_target).expect("read preexisting target"),
            b"existing content"
        );
    }

    #[test]
    fn extract_zip_archive_cleans_deeply_nested_partial_output() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");

        let file = fs::File::create(&zip_path).expect("create zip file");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("a/b/c/d/file.txt", SimpleFileOptions::default())
            .expect("start file entry");
        writer.write_all(b"payload").expect("write payload");
        writer
            .add_symlink(
                "a/b/c/d/bad-link",
                "target.exe",
                SimpleFileOptions::default(),
            )
            .expect("add symlink entry");
        writer.finish().expect("finish zip file");

        let error = extract_zip_archive(&zip_path, &destination_dir)
            .expect_err("expected cleanup after nested failure");

        assert!(
            error
                .to_string()
                .contains("refusing to extract symlink entry")
        );
        assert!(destination_dir.exists());
        assert!(!destination_dir.join("a").exists());
    }

    #[test]
    fn extract_zip_archive_rejects_suspicious_compression_ratio() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_zip_archive(&zip_path, "payload.txt", b"compressible payload");

        let error = extract_zip_archive_with_limits(
            &zip_path,
            &destination_dir,
            ExtractionLimits {
                max_total_size: 10 * 1024 * 1024 * 1024,
                max_file_count: 100_000,
                max_compression_ratio: 0,
                max_path_depth: 255,
            },
        )
        .expect_err("expected suspicious compression ratio rejection");

        assert!(error.to_string().contains("suspicious compression ratio"));
        assert!(!destination_dir.join("payload.txt").exists());
    }

    #[test]
    fn extract_zip_archive_rejects_total_size_limit() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_zip_archive(&zip_path, "payload.txt", b"abcd");

        let error = extract_zip_archive_with_limits(
            &zip_path,
            &destination_dir,
            ExtractionLimits {
                max_total_size: 3,
                max_file_count: 100_000,
                max_compression_ratio: 100,
                max_path_depth: 255,
            },
        )
        .expect_err("expected quota rejection");

        assert!(error.to_string().contains("quota exceeded"));
        assert!(!destination_dir.join("payload.txt").exists());
    }

    #[test]
    fn extract_zip_archive_rejects_file_count_limit() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_archive_with_entries(&zip_path, &[("first.txt", b""), ("second.txt", b"")]);

        let error = extract_zip_archive_with_limits(
            &zip_path,
            &destination_dir,
            ExtractionLimits {
                max_total_size: 10 * 1024 * 1024 * 1024,
                max_file_count: 1,
                max_compression_ratio: 100,
                max_path_depth: 255,
            },
        )
        .expect_err("expected file count rejection");

        assert!(error.to_string().contains("entry count exceeded"));
        assert!(!destination_dir.join("first.txt").exists());
    }

    #[test]
    fn extract_zip_archive_rejects_path_depth_limit() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let zip_path = temp_dir.path().join("archive.zip");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_zip_archive(&zip_path, "a/b/c/file.txt", b"payload");

        let error = extract_zip_archive_with_limits(
            &zip_path,
            &destination_dir,
            ExtractionLimits {
                max_total_size: 10 * 1024 * 1024 * 1024,
                max_file_count: 100_000,
                max_compression_ratio: 100,
                max_path_depth: 2,
            },
        )
        .expect_err("expected path depth rejection");

        assert!(error.to_string().contains("too deep"));
        assert!(!destination_dir.join("a").exists());
    }
}
