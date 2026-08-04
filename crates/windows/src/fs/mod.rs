use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use std::os::windows::fs::OpenOptionsExt;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};

use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

/// Metadata snapshot returned by [`inspect_path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathInfo {
    /// `true` when the path points to a directory.
    pub is_directory: bool,
    /// `true` when the path is marked as a reparse point.
    pub is_reparse_point: bool,
    /// Number of hard links attached to the path entry.
    pub hard_link_count: u32,
}

/// Inspect a Windows filesystem path without following reparse points.
///
/// The helper opens the target with the Windows handle APIs, reads handle
/// information, and returns the small metadata set WinBrew needs for extraction
/// and cleanup decisions.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use winbrew_windows::fs::inspect_path;
///
/// let info = inspect_path(Path::new(r"C:\Temp\payload.msix")).unwrap();
/// println!("dir={} reparse={} links={}", info.is_directory, info.is_reparse_point, info.hard_link_count);
/// ```
pub fn inspect_path(path: &Path) -> io::Result<PathInfo> {
    use std::mem::MaybeUninit;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
        OPEN_EXISTING,
    };

    let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide_path.push(0);

    unsafe {
        let handle = CreateFileW(
            wide_path.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        );

        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let _handle_guard = HandleGuard(handle);

        let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        if GetFileInformationByHandle(handle, info.as_mut_ptr()) == 0 {
            return Err(io::Error::last_os_error());
        }

        let info = info.assume_init();

        Ok(PathInfo {
            is_directory: info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
            is_reparse_point: info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
            hard_link_count: info.nNumberOfLinks,
        })
    }
}

/// Create a new file for extraction, failing if the target already exists.
///
/// This helper is used by archive and package extraction code when the output
/// path must be brand new. It keeps the file creation rules in one place and
/// applies the Windows flag WinBrew expects for reparse-point-aware staging.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use winbrew_windows::fs::create_extraction_target_file;
///
/// let _file = create_extraction_target_file(Path::new(r"C:\Temp\extract\tool.exe")).unwrap();
/// ```
pub fn create_extraction_target_file(path: &Path) -> io::Result<fs::File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[deprecated(note = "use create_extraction_target_file")]
#[doc(hidden)]
pub fn create_extracted_file(path: &Path) -> io::Result<fs::File> {
    create_extraction_target_file(path)
}

/// Reports whether `left` and `right` name the same filesystem location.
///
/// Install-location comparisons (MSI's registry-reported `InstallLocation`,
/// native-exe's uninstall-registry `InstallLocation`) need to tolerate a
/// canonicalization failure: the recorded path may not exist yet, or may no
/// longer exist right after an uninstall, both of which are normal at the
/// point this comparison runs. `fs::canonicalize` is tried first since it
/// correctly resolves symlinks/junctions and 8.3 short names; if either side
/// can't be canonicalized, this falls back to a lossy, case-insensitive,
/// separator-normalized string comparison, which is a strictly weaker check
/// (it can't detect distinct paths that alias to the same location) but is
/// the best available signal when the path isn't resolvable on disk.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use winbrew_windows::fs::paths_refer_to_same_location;
///
/// assert!(paths_refer_to_same_location(
///     Path::new(r"C:\Tools\App"),
///     Path::new(r"C:\Tools\App\"),
/// ));
/// ```
pub fn paths_refer_to_same_location(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => normalize_path_text(left) == normalize_path_text(right),
    }
}

fn normalize_path_text(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::paths_refer_to_same_location;
    use std::path::Path;

    #[test]
    fn matches_existing_directories_via_canonicalization() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let trailing_slash = temp_dir.path().join("");

        assert!(paths_refer_to_same_location(
            temp_dir.path(),
            &trailing_slash
        ));
    }

    #[test]
    fn falls_back_to_lossy_text_comparison_for_missing_paths() {
        assert!(paths_refer_to_same_location(
            Path::new(r"C:\Tools\App"),
            Path::new(r"c:\tools\app\"),
        ));
        assert!(paths_refer_to_same_location(
            Path::new(r"C:\Tools\App"),
            Path::new("C:/Tools/App"),
        ));
        assert!(!paths_refer_to_same_location(
            Path::new(r"C:\Tools\App"),
            Path::new(r"C:\Tools\OtherApp"),
        ));
    }
}
