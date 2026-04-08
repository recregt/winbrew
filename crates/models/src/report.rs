use std::time::Duration;

use crate::diagnostics::DiagnosisResult;

#[derive(Debug, Clone, PartialEq, Eq)]
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
