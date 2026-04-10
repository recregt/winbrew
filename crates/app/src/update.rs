use anyhow::Result;

use crate::core::paths::ResolvedPaths;

pub fn refresh_catalog<FStart, FProgress>(
    paths: &ResolvedPaths,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    crate::install_crate::catalog::refresh_catalog(paths, on_start, on_progress)
}
