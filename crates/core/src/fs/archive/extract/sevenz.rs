use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::fs::{FsError, Result};

const SEVENZ_HOME_ENV: &str = "WINBREW_7ZIP_HOME";
const SEVENZ_RELATIVE_EXE: &str = "bin/7zip/7z.exe";
const SEVENZ_RELATIVE_DLL: &str = "bin/7zip/7z.dll";

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
    let runtime_root = resolve_runtime_root().map_err(|err| {
        FsError::archive_backend_failed("7z", archive_path, Path::new(SEVENZ_RELATIVE_EXE), err)
    })?;

    extract_sevenz_with_runtime_root(
        archive_path,
        destination_dir,
        &runtime_root,
        &SystemSevenZipLauncher,
    )
}

pub(crate) fn sevenz_bin_path_from_runtime_root(runtime_root: &Path) -> PathBuf {
    runtime_root.join(SEVENZ_RELATIVE_EXE)
}

pub(crate) fn sevenz_dll_path_from_runtime_root(runtime_root: &Path) -> PathBuf {
    runtime_root.join(SEVENZ_RELATIVE_DLL)
}

pub(crate) fn extract_sevenz_with_runtime_root<L: SevenZipLauncher>(
    archive_path: &Path,
    destination_dir: &Path,
    runtime_root: &Path,
    launcher: &L,
) -> Result<()> {
    let binary_path = sevenz_bin_path_from_runtime_root(runtime_root);
    let dll_path = sevenz_dll_path_from_runtime_root(runtime_root);

    if !binary_path.exists() {
        let missing_binary_error = io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing 7z binary at {}", binary_path.display()),
        );
        return Err(FsError::archive_backend_failed(
            "7z",
            archive_path,
            &binary_path,
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
        .extract(&binary_path, archive_path, destination_dir)
        .map_err(|err| FsError::archive_backend_failed("7z", archive_path, &binary_path, err))?;

    Ok(())
}

fn resolve_runtime_root() -> io::Result<PathBuf> {
    if let Some(runtime_root) = env::var_os(SEVENZ_HOME_ENV) {
        return Ok(PathBuf::from(runtime_root));
    }

    let mut executable_path = env::current_exe()?;
    executable_path.pop();
    Ok(executable_path)
}
