use anyhow::Result;

use crate::AppContext;
use crate::app::repair;

pub fn run(ctx: &AppContext, yes: bool) -> Result<()> {
    repair::run(ctx, yes)
}
