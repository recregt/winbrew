use anyhow::Result;

use crate::AppContext;

pub fn refresh_catalog<FStart, FProgress>(
    ctx: &AppContext,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    crate::catalog::refresh_catalog(&ctx.paths, on_start, on_progress)
}
