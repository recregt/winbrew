//! Secure ZIP archive extraction helpers.
//!
//! The extractor validates ancestor paths, rejects symlink entries, and
//! best-effort cleans up anything it created if extraction fails halfway through.

use anyhow::{Context, Result};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use super::cleanup::inspect_path;
use super::cleanup_path;

/// Extracts `zip_path` into `destination_dir`, rejecting entries with invalid paths.
///
/// The extraction target is validated so the archive cannot be unpacked through
/// an existing reparse-point ancestor, and symlink entries are refused.
pub fn extract_zip_archive(zip_path: &Path, destination_dir: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("failed to open zip archive {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("failed to open zip archive")?;
    const ZIP_COPY_BUFFER_SIZE: usize = 256 * 1024;
    let mut cleanup = ExtractionCleanup::new();
    let mut buffer = vec![0u8; ZIP_COPY_BUFFER_SIZE];

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("failed to read zip entry")?;
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

        validate_extraction_target(&outpath)?;

        if entry.is_dir() {
            create_directory_tree(&outpath, &mut cleanup)?;
            continue;
        }

        if let Some(parent) = outpath.parent() {
            create_directory_tree(parent, &mut cleanup)?;
        }

        let mut outfile = fs::File::create(&outpath)
            .with_context(|| format!("failed to create extracted file {}", outpath.display()))?;
        cleanup.record(outpath.clone());

        loop {
            let bytes_read = entry
                .read(&mut buffer)
                .with_context(|| format!("failed to read zip entry {}", outpath.display()))?;
            if bytes_read == 0 {
                break;
            }

            outfile
                .write_all(&buffer[..bytes_read])
                .with_context(|| format!("failed to extract {}", outpath.display()))?;
        }
    }

    cleanup.commit();
    Ok(())
}

fn create_directory_tree(path: &Path, cleanup: &mut ExtractionCleanup) -> Result<()> {
    let mut missing_directories = Vec::new();
    let mut current = Some(path);

    while let Some(candidate) = current {
        match inspect_path(candidate) {
            Ok(info) => {
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
            Err(err) if err.kind() == ErrorKind::NotFound => {
                missing_directories.push(candidate.to_path_buf());
                current = candidate.parent();
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to inspect {}", candidate.display()));
            }
        }
    }

    for directory in missing_directories.iter().rev() {
        fs::create_dir(directory)
            .with_context(|| format!("failed to create directory {}", directory.display()))?;
        cleanup.record(directory.clone());
    }

    Ok(())
}

fn validate_extraction_target(path: &Path) -> Result<()> {
    let mut current = Some(path);

    while let Some(candidate) = current {
        match inspect_path(candidate) {
            Ok(info) => {
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
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to inspect {}", candidate.display()));
            }
        }

        current = candidate.parent();
    }

    Ok(())
}

struct ExtractionCleanup {
    created_paths: Vec<PathBuf>,
    committed: bool,
}

impl ExtractionCleanup {
    fn new() -> Self {
        Self {
            created_paths: Vec::new(),
            committed: false,
        }
    }

    fn record(&mut self, path: PathBuf) {
        self.created_paths.push(path);
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for ExtractionCleanup {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

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
}
