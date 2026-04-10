use anyhow::Result;

use crate::core::paths::ResolvedPaths;
use crate::models::{ConfigSection, InfoReport};
use crate::report;
use crate::version;

pub fn collect(sections: &[ConfigSection], resolved_paths: &ResolvedPaths) -> Result<InfoReport> {
    Ok(InfoReport {
        version: version::version_string(),
        runtime: report::runtime_report(sections, resolved_paths)?,
    })
}
