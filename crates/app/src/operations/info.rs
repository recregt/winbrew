use anyhow::Result;

use crate::core::paths::ResolvedPaths;
use crate::models::domains::reporting::InfoReport;
use crate::models::domains::shared::ConfigSection;
use crate::report;
use crate::version;

pub fn collect(sections: &[ConfigSection], resolved_paths: &ResolvedPaths) -> Result<InfoReport> {
    Ok(InfoReport {
        version: version::version_string(),
        runtime: report::runtime_report(sections, resolved_paths)?,
    })
}
