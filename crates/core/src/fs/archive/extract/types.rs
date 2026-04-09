#[derive(Debug, Clone, Copy)]
pub(crate) struct PathInfo {
    pub(crate) is_directory: bool,
    pub(crate) is_reparse_point: bool,
    pub(crate) hard_link_count: u32,
}

#[derive(Clone, Copy)]
pub(crate) enum CachedPath {
    Missing,
    Present(PathInfo),
}
