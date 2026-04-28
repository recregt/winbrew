use std::io;
use std::path::Path;

use super::super::extract::PathInfo;
use super::{PlatformAdapter, WindowsPlatform};

impl PlatformAdapter for WindowsPlatform {
    fn inspect_path(path: &Path) -> io::Result<PathInfo> {
        winbrew_windows::paths::inspect_path(path).map(|info| PathInfo {
            is_directory: info.is_directory,
            is_reparse_point: info.is_reparse_point,
            hard_link_count: info.hard_link_count,
        })
    }

    fn create_extracted_file(path: &Path) -> io::Result<std::fs::File> {
        winbrew_windows::paths::create_extracted_file(path)
    }
}
