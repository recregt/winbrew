use anyhow::Result;

use crate::AppContext;
use crate::services::report::{RuntimeReport, runtime_report as report_runtime_report};

pub fn runtime_report(ctx: &AppContext) -> Result<RuntimeReport> {
    report_runtime_report(ctx)
}
