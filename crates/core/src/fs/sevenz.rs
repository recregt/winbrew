use std::path::{Path, PathBuf};

#[cfg(windows)]
use winbrew_windows::host::search_path_file;

pub fn sevenz_bin_path_from_runtime_root(runtime_root: &Path) -> PathBuf {
    sevenz_runtime_dir_from_runtime_root(runtime_root).join("7z.exe")
}

pub fn sevenz_dll_path_from_runtime_root(runtime_root: &Path) -> PathBuf {
    sevenz_runtime_dir_from_runtime_root(runtime_root).join("7z.dll")
}

pub fn sevenz_runtime_dir_from_runtime_root(runtime_root: &Path) -> PathBuf {
    runtime_root.join("bin/7zip")
}

#[cfg(windows)]
pub fn system_sevenz_binary_path() -> Option<PathBuf> {
    search_path_file("7z.exe").and_then(|binary_path| {
        let runtime_root = binary_path.parent()?;

        if runtime_root.join("7z.dll").exists() {
            Some(binary_path)
        } else {
            None
        }
    })
}

#[cfg(not(windows))]
pub fn system_sevenz_binary_path() -> Option<PathBuf> {
    None
}
