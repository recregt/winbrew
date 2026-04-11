use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use std::os::windows::fs::OpenOptionsExt;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};

use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathInfo {
    pub is_directory: bool,
    pub is_reparse_point: bool,
    pub hard_link_count: u32,
}

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

pub fn create_extracted_file(path: &Path) -> io::Result<fs::File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
