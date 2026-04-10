use serde::{Serialize, Serializer};
use std::time::Duration;

use crate::diagnostics::DiagnosisResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthReport {
    pub database_path: String,
    pub database_exists: bool,
    pub catalog_database_path: String,
    pub catalog_database_exists: bool,
    pub install_root_source: String,
    pub install_root: String,
    pub install_root_exists: bool,
    pub packages_dir: String,
    pub diagnostics: Vec<DiagnosisResult>,
    #[serde(serialize_with = "serialize_duration_millis")]
    pub scan_duration: Duration,
    pub error_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub sections: Vec<ReportSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSection {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

impl RuntimeReport {
    pub fn new(sections: Vec<ReportSection>) -> Self {
        Self { sections }
    }
}

fn serialize_duration_millis<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let millis = duration.as_millis().min(u64::MAX as u128) as u64;
    serializer.serialize_u64(millis)
}
