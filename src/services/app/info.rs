use anyhow::Result;

use crate::{
    AppContext,
    services::{app::version, shared::report},
};
use winbrew_models::InfoReport;

pub fn collect(ctx: &AppContext) -> Result<InfoReport> {
    Ok(InfoReport {
        version: version::version_string(),
        runtime: report::runtime_report(&ctx.sections, &ctx.paths)?,
    })
}
