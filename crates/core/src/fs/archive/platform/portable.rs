use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use super::super::extract::PathInfo;
use super::{PlatformAdapter, PortablePlatform};

impl PlatformAdapter for PortablePlatform {
    fn inspect_path(path: &Path) -> io::Result<PathInfo> {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::symlink_metadata(path)?;
        Ok(PathInfo {
            is_directory: metadata.is_dir(),
            is_reparse_point: false,
            #[cfg(unix)]
            hard_link_count: metadata.nlink() as u32,
        })
    }

    fn create_extracted_file(path: &Path) -> io::Result<fs::File> {
        OpenOptions::new().write(true).create_new(true).open(path)
    }
}
