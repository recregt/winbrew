use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct PathInfo {
    pub(super) is_directory: bool,
    pub(super) is_reparse_point: bool,
    pub(super) hard_link_count: u32,
}

pub(super) fn inspect_path(path: &Path) -> std::io::Result<PathInfo> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(PathInfo {
        is_directory: metadata.is_dir(),
        is_reparse_point: false,
        hard_link_count: 1,
    })
}
