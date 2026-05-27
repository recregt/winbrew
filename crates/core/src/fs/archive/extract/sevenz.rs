use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::env::{LOCALAPPDATA, WINBREW_PATHS_ROOT};
use crate::fs::{FsError, Result, system_sevenz_binary_path};

pub(crate) use crate::fs::{sevenz_bin_path_from_runtime_root, sevenz_dll_path_from_runtime_root};

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
    #[cfg(windows)]
    {
        if let Some(system_binary_path) = system_sevenz_binary_path() {
            return extract_sevenz_with_binary_path(
                archive_path,
                destination_dir,
                &system_binary_path,
                &SystemSevenZipLauncher,
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
    )
}

pub(crate) fn extract_sevenz_with_runtime_root<L: SevenZipLauncher>(
    archive_path: &Path,
    destination_dir: &Path,
    runtime_root: &Path,
    launcher: &L,
) -> Result<()> {
    let binary_path = sevenz_bin_path_from_runtime_root(runtime_root);
    let _dll_path = sevenz_dll_path_from_runtime_root(runtime_root);
    extract_sevenz_with_binary_path(archive_path, destination_dir, &binary_path, launcher)
}

pub(crate) fn extract_sevenz_with_binary_path<L: SevenZipLauncher>(
    archive_path: &Path,
    destination_dir: &Path,
    binary_path: &Path,
    launcher: &L,
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

        let error = extract_sevenz_with_runtime_root(
            &archive_path,
            &destination_dir,
            &runtime_root,
            &launcher,
        )
        .expect_err("expected missing binary rejection");

        assert!(error.to_string().contains("failed to extract 7z archive"));
        assert!(launcher.calls.borrow().is_empty());
    }
}
