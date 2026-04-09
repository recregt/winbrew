use std::fs;
use std::io;
use std::path::Path;

use super::extract::PathInfo;

pub(super) trait PlatformAdapter {
    fn inspect_path(path: &Path) -> io::Result<PathInfo>;

    fn create_extracted_file(path: &Path) -> io::Result<fs::File>;
}

#[cfg(not(windows))]
mod portable;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(super) struct WindowsPlatform;

#[cfg(not(windows))]
pub(super) struct PortablePlatform;
