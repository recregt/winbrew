use super::sevenz::{
    SevenZipLauncher, extract_sevenz_with_runtime_root, sevenz_bin_path_from_runtime_root,
    sevenz_dll_path_from_runtime_root,
};
use super::{
    ExtractionLimits, extract_archive, extract_zip_archive, extract_zip_archive_with_limits,
};
use crate::fs::ArchiveKind;
use bzip2::Compression as BzCompression;
use bzip2::write::BzEncoder;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::cell::RefCell;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;
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

fn create_tar_archive(path: &std::path::Path, file_name: &str, contents: &[u8]) {
    let file = fs::File::create(path).expect("create tar file");
    let mut builder = Builder::new(file);
    let mut header = Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();

    builder
        .append_data(&mut header, file_name, contents)
        .expect("append tar entry");
    builder.finish().expect("finish tar file");
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

fn create_gz_archive(path: &std::path::Path, contents: &[u8]) {
    let file = fs::File::create(path).expect("create gz file");
    let mut encoder = GzEncoder::new(file, Compression::default());

    encoder.write_all(contents).expect("write gz contents");
    encoder.finish().expect("finish gz file");
}

fn create_tar_bz2_archive(path: &std::path::Path, file_name: &str, contents: &[u8]) {
    let file = fs::File::create(path).expect("create tar.bz2 file");
    let encoder = BzEncoder::new(file, BzCompression::default());
    let mut builder = Builder::new(encoder);
    let mut header = Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();

    builder
        .append_data(&mut header, file_name, contents)
        .expect("append tar.bz2 entry");
    let encoder = builder.into_inner().expect("finish tar builder");
    encoder.finish().expect("finish tar.bz2 file");
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

    let error =
        extract_zip_archive(&zip_path, &destination_dir).expect_err("expected symlink rejection");

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

#[test]
fn extract_archive_extracts_gzip_archive() {
    let temp_dir = tempdir().expect("temp dir");
    let destination_dir = temp_dir.path().join("dest");
    let archive_path = temp_dir.path().join("tool.exe.gz");

    fs::create_dir_all(&destination_dir).expect("destination dir");
    create_gz_archive(&archive_path, b"gzip payload");

    extract_archive(ArchiveKind::Gzip, &archive_path, &destination_dir).expect("gzip extraction");

    assert_eq!(
        fs::read(destination_dir.join("tool.exe")).expect("read"),
        b"gzip payload"
    );
}

#[test]
fn sevenz_runtime_layout_uses_expected_relative_paths() {
    let runtime_root = PathBuf::from("C:/winbrew");

    assert_eq!(
        sevenz_bin_path_from_runtime_root(&runtime_root),
        PathBuf::from("C:/winbrew/bin/7zip/7z.exe")
    );
    assert_eq!(
        sevenz_dll_path_from_runtime_root(&runtime_root),
        PathBuf::from("C:/winbrew/bin/7zip/7z.dll")
    );
}

struct RecordingSevenZipLauncher {
    calls: RefCell<Vec<(PathBuf, PathBuf, PathBuf)>>,
}

impl RecordingSevenZipLauncher {
    fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl SevenZipLauncher for RecordingSevenZipLauncher {
    fn extract(
        &self,
        binary_path: &std::path::Path,
        archive_path: &std::path::Path,
        destination_dir: &std::path::Path,
    ) -> io::Result<()> {
        self.calls.borrow_mut().push((
            binary_path.to_path_buf(),
            archive_path.to_path_buf(),
            destination_dir.to_path_buf(),
        ));

        Ok(())
    }
}

#[test]
fn extract_sevenz_uses_runtime_root_and_launcher() {
    let temp_dir = tempdir().expect("temp dir");
    let runtime_root = temp_dir.path().join("runtime");
    let archive_path = temp_dir.path().join("archive.7z");
    let destination_dir = temp_dir.path().join("dest");
    let launcher = RecordingSevenZipLauncher::new();
    let binary_path = sevenz_bin_path_from_runtime_root(&runtime_root);
    let dll_path = sevenz_dll_path_from_runtime_root(&runtime_root);

    fs::create_dir_all(binary_path.parent().expect("binary parent")).expect("binary dir");
    fs::write(&binary_path, b"placeholder").expect("fake binary");
    fs::write(&dll_path, b"placeholder").expect("fake dll");
    fs::write(&archive_path, b"archive contents").expect("archive file");

    extract_sevenz_with_runtime_root(&archive_path, &destination_dir, &runtime_root, &launcher)
        .expect("sevenzip extraction");

    let calls = launcher.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, binary_path);
    assert_eq!(calls[0].1, archive_path);
    assert_eq!(calls[0].2, destination_dir);
}

#[test]
fn extract_sevenz_rejects_missing_binary_before_launch() {
    let temp_dir = tempdir().expect("temp dir");
    let runtime_root = temp_dir.path().join("runtime");
    let archive_path = temp_dir.path().join("archive.7z");
    let destination_dir = temp_dir.path().join("dest");
    let launcher = RecordingSevenZipLauncher::new();

    fs::create_dir_all(&runtime_root).expect("runtime dir");
    fs::write(&archive_path, b"archive contents").expect("archive file");

    let error =
        extract_sevenz_with_runtime_root(&archive_path, &destination_dir, &runtime_root, &launcher)
            .expect_err("expected missing binary rejection");

    assert!(error.to_string().contains("failed to extract 7z archive"));
    assert!(launcher.calls.borrow().is_empty());
}

#[test]
fn extract_archive_extracts_tar_archive() {
    let temp_dir = tempdir().expect("temp dir");
    let destination_dir = temp_dir.path().join("dest");
    let archive_path = temp_dir.path().join("archive.tar");

    fs::create_dir_all(&destination_dir).expect("destination dir");
    create_tar_archive(&archive_path, "bin/tool.exe", b"tar payload");

    extract_archive(ArchiveKind::Tar, &archive_path, &destination_dir).expect("tar extraction");

    assert_eq!(
        fs::read(destination_dir.join("bin/tool.exe")).expect("read"),
        b"tar payload"
    );
}

#[test]
fn extract_archive_extracts_tar_gz_archive() {
    let temp_dir = tempdir().expect("temp dir");
    let destination_dir = temp_dir.path().join("dest");
    let archive_path = temp_dir.path().join("archive.tar.gz");

    fs::create_dir_all(&destination_dir).expect("destination dir");
    create_tar_gz_archive(&archive_path, "bin/tool.exe", b"tar gz payload");

    extract_archive(ArchiveKind::Tar, &archive_path, &destination_dir).expect("tar.gz extraction");

    assert_eq!(
        fs::read(destination_dir.join("bin/tool.exe")).expect("read"),
        b"tar gz payload"
    );
}

#[test]
fn extract_archive_extracts_tar_bz2_archive() {
    let temp_dir = tempdir().expect("temp dir");
    let destination_dir = temp_dir.path().join("dest");
    let archive_path = temp_dir.path().join("archive.tar.bz2");

    fs::create_dir_all(&destination_dir).expect("destination dir");
    create_tar_bz2_archive(&archive_path, "bin/tool.exe", b"tar bz2 payload");

    extract_archive(ArchiveKind::Tar, &archive_path, &destination_dir).expect("tar.bz2 extraction");

    assert_eq!(
        fs::read(destination_dir.join("bin/tool.exe")).expect("read"),
        b"tar bz2 payload"
    );
}
