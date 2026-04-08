//! Secure ZIP archive extraction helpers.
//!
//! # Security Features
//! - Path traversal prevention via `enclosed_name()` validation
//! - Reparse point ancestor detection
//! - Hard link overwrite protection
//! - Symlink entry rejection
//! - RAII cleanup on failure
//!
//! # Performance Features
//! - Ancestor path inspection caching
//! - Reusable extraction buffer
//!
//! The extractor validates ancestor paths, rejects symlink entries, caches
//! ancestor inspection results, and best-effort cleans up anything it created if
//! extraction fails halfway through.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use super::cleanup::PathInfo;
use super::cleanup::inspect_path;
use super::cleanup_path;

#[derive(Clone, Copy)]
enum CachedPath {
    Missing,
    Present(PathInfo),
}

/// Extracts `zip_path` into `destination_dir`, rejecting entries with invalid paths.
///
/// The extraction target is validated so the archive cannot be unpacked through
/// an existing reparse-point ancestor, and symlink entries are refused.
pub fn extract_zip_archive(zip_path: &Path, destination_dir: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("failed to open zip archive {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("failed to open zip archive")?;
    const ZIP_COPY_BUFFER_SIZE: usize = 256 * 1024;
    let mut extraction = ExtractionContext::new();
    let mut buffer = vec![0u8; ZIP_COPY_BUFFER_SIZE];

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("failed to read zip entry")?;
        extract_entry(&mut entry, destination_dir, &mut extraction, &mut buffer)?;
    }

    extraction.commit();
    Ok(())
}

struct ExtractionContext {
    cached_paths: HashMap<PathBuf, CachedPath>,
    cleanup: ExtractionCleanup,
}

impl ExtractionContext {
    fn new() -> Self {
        Self {
            cached_paths: HashMap::new(),
            cleanup: ExtractionCleanup::new(),
        }
    }

    fn commit(self) {
        self.cleanup.commit();
    }

    fn validate_target(&mut self, path: &Path) -> Result<()> {
        let mut current = Some(path);

        while let Some(candidate) = current {
            match self.inspect_cached(candidate)? {
                CachedPath::Present(info) => {
                    if info.is_reparse_point {
                        return Err(anyhow::anyhow!(
                            "refusing to extract through reparse point {}",
                            candidate.display()
                        ));
                    }

                    if !info.is_directory && info.hard_link_count > 1 {
                        return Err(anyhow::anyhow!(
                            "refusing to overwrite hardlinked file {}",
                            candidate.display()
                        ));
                    }
                }
                CachedPath::Missing => {}
            }

            current = candidate.parent();
        }

        Ok(())
    }

    fn ensure_directory_tree(&mut self, path: &Path) -> Result<()> {
        let mut missing_directories = Vec::new();
        let mut current = Some(path);

        while let Some(candidate) = current {
            match self.inspect_cached(candidate)? {
                CachedPath::Present(info) => {
                    if info.is_reparse_point {
                        return Err(anyhow::anyhow!(
                            "refusing to create directory through reparse point {}",
                            candidate.display()
                        ));
                    }

                    if !info.is_directory {
                        return Err(anyhow::anyhow!(
                            "failed to create directory {}: path exists and is not a directory",
                            candidate.display()
                        ));
                    }

                    break;
                }
                CachedPath::Missing => {
                    missing_directories.push(candidate.to_path_buf());
                    current = candidate.parent();
                }
            }
        }

        for directory in missing_directories.iter().rev() {
            fs::create_dir_all(directory)
                .with_context(|| format!("failed to create directory {}", directory.display()))?;
            self.record_directory(directory);
        }

        Ok(())
    }

    fn record_directory(&mut self, path: &Path) {
        self.cached_paths.insert(
            path.to_path_buf(),
            CachedPath::Present(PathInfo {
                is_directory: true,
                is_reparse_point: false,
                hard_link_count: 1,
            }),
        );
        self.cleanup.record(path.to_path_buf());
    }

    fn record_file(&mut self, path: &Path) {
        self.cached_paths.insert(
            path.to_path_buf(),
            CachedPath::Present(PathInfo {
                is_directory: false,
                is_reparse_point: false,
                hard_link_count: 1,
            }),
        );
        self.cleanup.record(path.to_path_buf());
    }

    fn inspect_cached(&mut self, path: &Path) -> Result<CachedPath> {
        if let Some(cached) = self.cached_paths.get(path) {
            return Ok(*cached);
        }

        let state = match inspect_path(path) {
            Ok(info) => CachedPath::Present(info),
            Err(err) if err.kind() == ErrorKind::NotFound => CachedPath::Missing,
            Err(err) => {
                return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
            }
        };

        self.cached_paths.insert(path.to_path_buf(), state);
        Ok(state)
    }
}

struct ExtractionCleanup {
    created_paths: Vec<PathBuf>,
}

impl ExtractionCleanup {
    fn new() -> Self {
        Self {
            created_paths: Vec::new(),
        }
    }

    fn record(&mut self, path: PathBuf) {
        self.created_paths.push(path);
    }

    fn commit(mut self) {
        self.created_paths.clear();
    }
}

fn extract_entry<R: Read>(
    entry: &mut zip::read::ZipFile<'_, R>,
    destination_dir: &Path,
    extraction: &mut ExtractionContext,
    buffer: &mut [u8],
) -> Result<()> {
    let enclosed_name = entry
        .enclosed_name()
        .ok_or_else(|| anyhow::anyhow!("zip entry contains an invalid path"))?;
    let outpath = destination_dir.join(enclosed_name);

    if entry.is_symlink() {
        return Err(anyhow::anyhow!(
            "refusing to extract symlink entry {}",
            outpath.display()
        ));
    }

    extraction.validate_target(&outpath)?;

    if entry.is_dir() {
        extraction.ensure_directory_tree(&outpath)?;
        return Ok(());
    }

    if let Some(parent) = outpath.parent() {
        extraction.ensure_directory_tree(parent)?;
    }

    let mut outfile = fs::File::create(&outpath)
        .with_context(|| format!("failed to create extracted file {}", outpath.display()))?;
    extraction.record_file(&outpath);

    loop {
        let bytes_read = entry
            .read(buffer)
            .with_context(|| format!("failed to read zip entry {}", outpath.display()))?;
        if bytes_read == 0 {
            break;
        }

        outfile
            .write_all(&buffer[..bytes_read])
            .with_context(|| format!("failed to extract {}", outpath.display()))?;
    }

    Ok(())
}

impl Drop for ExtractionCleanup {
    fn drop(&mut self) {
        while let Some(path) = self.created_paths.pop() {
            let _ = cleanup_path(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extract_zip_archive;
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
            fs::read(&destination_dir.join("bin/tool.exe")).expect("read"),
            b"binary content"
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
}
