use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct PathInfo {
    pub(super) is_directory: bool,
    pub(super) is_reparse_point: bool,
    pub(super) hard_link_count: u32,
}

pub(super) fn inspect_path(path: &Path) -> std::io::Result<PathInfo> {
    let info = winbrew_windows::inspect_path(path)?;
    Ok(PathInfo {
        is_directory: info.is_directory,
        is_reparse_point: info.is_reparse_point,
        hard_link_count: info.hard_link_count,
    })
}
