use anyhow::Result;

use crate::{
    AppContext,
    models::InfoReport,
    services::{app::version, shared::report},
};

pub fn collect(ctx: &AppContext) -> Result<InfoReport> {
    Ok(InfoReport {
        version: version::version_string(),
        runtime: report::runtime_report(&ctx.sections, &ctx.paths)?,
    })
}
