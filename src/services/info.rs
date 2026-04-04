use anyhow::Result;

use crate::services::report::{RuntimeReport, runtime_report as report_runtime_report};

pub fn runtime_report() -> Result<RuntimeReport> {
    report_runtime_report()
}
