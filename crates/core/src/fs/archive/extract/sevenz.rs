use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::env::{LOCALAPPDATA, WINBREW_PATHS_ROOT};
use crate::fs::{FsError, Result};
use crate::paths::{
    sevenz_bin_path_from_runtime_root, sevenz_dll_path_from_runtime_root, system_sevenz_binary_path,
};

use super::super::context::ExtractionContext;
use super::super::limits::ExtractionLimits;
use super::super::platform::PlatformAdapter;
#[cfg(not(windows))]
use super::super::platform::PortablePlatform as DefaultPlatform;
#[cfg(windows)]
use super::super::platform::WindowsPlatform as DefaultPlatform;

const SEVENZ_RELATIVE_EXE: &str = "bin/7zip/7z.exe";

pub(crate) trait SevenZipLauncher {
    fn extract(
        &self,
        binary_path: &Path,
        archive_path: &Path,
        destination_dir: &Path,
    ) -> io::Result<()>;
}

pub(crate) struct SystemSevenZipLauncher;

impl SevenZipLauncher for SystemSevenZipLauncher {
    fn extract(
        &self,
        binary_path: &Path,
        archive_path: &Path,
        destination_dir: &Path,
    ) -> io::Result<()> {
        let status = Command::new(binary_path)
            .arg("x")
            .arg("-y")
            .arg("-bd")
            .arg(format!("-o{}", destination_dir.display()))
            .arg(archive_path)
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!("7z exited with status {status}")))
        }
    }
}

pub(crate) fn extract_sevenz(archive_path: &Path, destination_dir: &Path) -> Result<()> {
    extract_sevenz_with_limits(archive_path, destination_dir, ExtractionLimits::default())
}

pub(crate) fn extract_sevenz_with_limits(
    archive_path: &Path,
    destination_dir: &Path,
    limits: ExtractionLimits,
) -> Result<()> {
    #[cfg(windows)]
    {
        if let Some(system_binary_path) = system_sevenz_binary_path() {
            return extract_sevenz_with_binary_path(
                archive_path,
                destination_dir,
                &system_binary_path,
                &SystemSevenZipLauncher,
                limits,
            );
        }
    }

    let runtime_root = resolve_local_runtime_root().map_err(|err| {
        FsError::archive_backend_failed("7z", archive_path, Path::new(SEVENZ_RELATIVE_EXE), err)
    })?;

    extract_sevenz_with_runtime_root(
        archive_path,
        destination_dir,
        &runtime_root,
        &SystemSevenZipLauncher,
        limits,
    )
}

pub(crate) fn extract_sevenz_with_runtime_root<L: SevenZipLauncher>(
    archive_path: &Path,
    destination_dir: &Path,
    runtime_root: &Path,
    launcher: &L,
    limits: ExtractionLimits,
) -> Result<()> {
    let binary_path = sevenz_bin_path_from_runtime_root(runtime_root);
    let _dll_path = sevenz_dll_path_from_runtime_root(runtime_root);
    extract_sevenz_with_binary_path(
        archive_path,
        destination_dir,
        &binary_path,
        launcher,
        limits,
    )
}

pub(crate) fn extract_sevenz_with_binary_path<L: SevenZipLauncher>(
    archive_path: &Path,
    destination_dir: &Path,
    binary_path: &Path,
    launcher: &L,
    limits: ExtractionLimits,
) -> Result<()> {
    let dll_path = binary_path.with_file_name("7z.dll");

    if !binary_path.exists() {
        let missing_binary_error = io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing 7z binary at {}", binary_path.display()),
        );
        return Err(FsError::archive_backend_failed(
            "7z",
            archive_path,
            binary_path,
            missing_binary_error,
        ));
    }

    if !dll_path.exists() {
        let missing_dll_error = io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing 7z runtime library at {}", dll_path.display()),
        );
        return Err(FsError::archive_backend_failed(
            "7z",
            archive_path,
            &dll_path,
            missing_dll_error,
        ));
    }

    fs::create_dir_all(destination_dir)
        .map_err(|err| FsError::create_directory(destination_dir, err))?;

    launcher
        .extract(binary_path, archive_path, destination_dir)
        .map_err(|err| FsError::archive_backend_failed("7z", archive_path, binary_path, err))?;

    // 7z.exe writes the whole payload directly into `destination_dir` with no
    // per-entry hook, unlike the ZIP/Tar backends which validate and quota
    // every entry before writing it. Validate the result instead: reject any
    // reparse point/symlink or hardlinked file the archive produced, and
    // enforce the same total-size/file-count/path-depth quotas. A
    // compression-ratio bomb check is not meaningful here (the per-entry
    // compressed size is not available after 7z has already decompressed
    // everything), so it is intentionally not part of this check; the other
    // quotas still bound how much a malicious payload can do.
    if let Err(err) = verify_extracted_tree::<DefaultPlatform>(destination_dir, limits) {
        if let Err(cleanup_err) = crate::fs::cleanup_path(destination_dir) {
            tracing::warn!(
                path = %destination_dir.display(),
                error = %cleanup_err,
                "failed to clean up 7z extraction output after validation failure"
            );
        }
        return Err(err);
    }

    Ok(())
}

/// Walks `destination_dir` after an external 7z extraction and enforces the
/// same containment and quota checks the ZIP/Tar backends apply to each
/// entry before writing it.
fn verify_extracted_tree<P: PlatformAdapter>(
    destination_dir: &Path,
    limits: ExtractionLimits,
) -> Result<()> {
    let mut extraction = ExtractionContext::<P>::new(limits);
    let mut pending_dirs = vec![destination_dir.to_path_buf()];

    while let Some(dir) = pending_dirs.pop() {
        let read_dir = fs::read_dir(&dir).map_err(|err| FsError::inspect(&dir, err))?;

        for entry in read_dir {
            let entry = entry.map_err(|err| FsError::inspect(&dir, err))?;
            let path = entry.path();
            let relative_path = path.strip_prefix(destination_dir).unwrap_or(&path);
            let metadata = entry
                .metadata()
                .map_err(|err| FsError::inspect(&path, err))?;

            extraction.validate_target(&path, destination_dir)?;

            if metadata.is_dir() {
                extraction.check_limits(relative_path, 0, 0)?;
                pending_dirs.push(path);
            } else {
                let entry_size = metadata.len();
                extraction.check_limits(relative_path, entry_size, entry_size)?;
            }
        }
    }

    extraction.commit();
    Ok(())
}

fn resolve_local_runtime_root() -> io::Result<PathBuf> {
    if let Some(runtime_root) = std::env::var_os(WINBREW_PATHS_ROOT) {
        return Ok(PathBuf::from(runtime_root));
    }

    let local_app_data = std::env::var_os(LOCALAPPDATA).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "LOCALAPPDATA is not set on this Windows session",
        )
    })?;

    Ok(PathBuf::from(local_app_data).join("winbrew"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;
    use tempfile::tempdir;

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

        extract_sevenz_with_runtime_root(
            &archive_path,
            &destination_dir,
            &runtime_root,
            &launcher,
            ExtractionLimits::default(),
        )
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

        let error = extract_sevenz_with_runtime_root(
            &archive_path,
            &destination_dir,
            &runtime_root,
            &launcher,
            ExtractionLimits::default(),
        )
        .expect_err("expected missing binary rejection");

        assert!(error.to_string().contains("failed to extract 7z archive"));
        assert!(launcher.calls.borrow().is_empty());
    }

    /// A fake launcher that writes a fixed set of files/entries into
    /// `destination_dir` instead of actually invoking 7z, so the
    /// post-extraction verification pass can be tested without a real 7z
    /// binary or archive.
    struct WritingSevenZipLauncher<F: Fn(&Path)> {
        write: F,
    }

    impl<F: Fn(&Path)> SevenZipLauncher for WritingSevenZipLauncher<F> {
        fn extract(
            &self,
            _binary_path: &Path,
            _archive_path: &Path,
            destination_dir: &Path,
        ) -> io::Result<()> {
            (self.write)(destination_dir);
            Ok(())
        }
    }

    fn extract_with_fake_7z_output(
        write: impl Fn(&Path),
        limits: ExtractionLimits,
    ) -> (Result<()>, PathBuf, PathBuf) {
        let temp_dir = tempdir().expect("temp dir");
        let runtime_root = temp_dir.path().join("runtime");
        let archive_path = temp_dir.path().join("archive.7z");
        let destination_dir = temp_dir.path().join("dest");
        let binary_path = sevenz_bin_path_from_runtime_root(&runtime_root);
        let dll_path = sevenz_dll_path_from_runtime_root(&runtime_root);

        fs::create_dir_all(binary_path.parent().expect("binary parent")).expect("binary dir");
        fs::write(&binary_path, b"placeholder").expect("fake binary");
        fs::write(&dll_path, b"placeholder").expect("fake dll");
        fs::write(&archive_path, b"archive contents").expect("archive file");

        let launcher = WritingSevenZipLauncher { write };
        let result = extract_sevenz_with_runtime_root(
            &archive_path,
            &destination_dir,
            &runtime_root,
            &launcher,
            limits,
        );

        (result, destination_dir, temp_dir.keep())
    }

    #[test]
    fn extract_sevenz_accepts_ordinary_output() {
        let (result, destination_dir, _temp_dir) = extract_with_fake_7z_output(
            |dest| {
                fs::create_dir_all(dest.join("bin")).expect("bin dir");
                fs::write(dest.join("bin/tool.exe"), b"payload").expect("write payload");
            },
            ExtractionLimits::default(),
        );

        result.expect("ordinary 7z output should be accepted");
        assert!(destination_dir.join("bin/tool.exe").exists());
    }

    #[test]
    #[cfg(unix)]
    fn extract_sevenz_rejects_symlink_entries_and_cleans_up() {
        let (result, destination_dir, _temp_dir) = extract_with_fake_7z_output(
            |dest| {
                fs::create_dir_all(dest.join("bin")).expect("bin dir");
                fs::write(dest.join("bin/ok.txt"), b"ok").expect("write ok entry");
                std::os::unix::fs::symlink("/etc/passwd", dest.join("bin/evil"))
                    .expect("create symlink entry");
            },
            ExtractionLimits::default(),
        );

        let error = result.expect_err("symlink output should be rejected");
        assert!(
            error.to_string().contains("reparse point") || error.to_string().contains("symlink")
        );
        assert!(
            !destination_dir.exists(),
            "extraction output should be cleaned up after a rejected symlink"
        );
    }

    #[test]
    fn extract_sevenz_rejects_total_size_limit_and_cleans_up() {
        let (result, destination_dir, _temp_dir) = extract_with_fake_7z_output(
            |dest| {
                fs::write(dest.join("payload.bin"), vec![0u8; 16]).expect("write payload");
            },
            ExtractionLimits {
                max_total_size: 4,
                max_file_count: 100_000,
                max_compression_ratio: 100,
                max_path_depth: 255,
            },
        );

        let error = result.expect_err("oversized 7z output should be rejected");
        assert!(error.to_string().contains("quota exceeded"));
        assert!(
            !destination_dir.exists(),
            "extraction output should be cleaned up after exceeding the size quota"
        );
    }

    #[test]
    fn extract_sevenz_rejects_file_count_limit() {
        let (result, destination_dir, _temp_dir) = extract_with_fake_7z_output(
            |dest| {
                fs::write(dest.join("first.txt"), b"a").expect("write first");
                fs::write(dest.join("second.txt"), b"b").expect("write second");
            },
            ExtractionLimits {
                max_total_size: 10 * 1024 * 1024 * 1024,
                max_file_count: 1,
                max_compression_ratio: 100,
                max_path_depth: 255,
            },
        );

        let error = result.expect_err("too many entries should be rejected");
        assert!(error.to_string().contains("entry count exceeded"));
        assert!(!destination_dir.exists());
    }

    #[test]
    fn extract_sevenz_rejects_path_depth_limit() {
        let (result, destination_dir, _temp_dir) = extract_with_fake_7z_output(
            |dest| {
                fs::create_dir_all(dest.join("a/b/c")).expect("nested dirs");
                fs::write(dest.join("a/b/c/file.txt"), b"payload").expect("write payload");
            },
            ExtractionLimits {
                max_total_size: 10 * 1024 * 1024 * 1024,
                max_file_count: 100_000,
                max_compression_ratio: 100,
                max_path_depth: 2,
            },
        );

        let error = result.expect_err("overly deep output should be rejected");
        assert!(error.to_string().contains("too deep"));
        assert!(!destination_dir.exists());
    }
}
