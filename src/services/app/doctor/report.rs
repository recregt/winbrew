use anyhow::Result;

use crate::AppContext;
use crate::services::shared::report::{HealthReport, health_report as report_health_report};

pub fn health_report(ctx: &AppContext) -> Result<HealthReport> {
    report_health_report(ctx)
}
