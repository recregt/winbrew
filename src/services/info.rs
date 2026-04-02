use anyhow::Result;

use crate::database::RuntimeReport;

pub fn runtime_report() -> Result<RuntimeReport> {
    crate::database::get_runtime_report()
}
