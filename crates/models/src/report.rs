use serde::{Deserialize, Serialize, Serializer};
use std::time::Duration;

use crate::diagnostics::DiagnosisResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryIssueKind {
    RecoveryTrailMissing,
    IncompleteInstall,
    Conflict,
    DiskDrift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryActionGroup {
    JournalReplay,
    OrphanCleanup,
    FileRestore,
    Reinstall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryFinding {
    pub error_code: String,
    pub issue_kind: RecoveryIssueKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_group: Option<RecoveryActionGroup>,
    pub description: String,
    pub severity: crate::diagnostics::DiagnosisSeverity,
}

impl RecoveryFinding {
    pub fn from_diagnosis(diagnosis: &DiagnosisResult) -> Option<Self> {
        let (issue_kind, action_group) = match diagnosis.error_code.as_str() {
            "missing_install_directory" | "install_directory_not_a_directory" => (
                RecoveryIssueKind::DiskDrift,
                Some(RecoveryActionGroup::Reinstall),
            ),
            "install_directory_permission_denied" | "install_directory_unreadable" => (
                RecoveryIssueKind::DiskDrift,
                Some(RecoveryActionGroup::Reinstall),
            ),
            "missing_msi_file"
            | "msi_file_not_a_file"
            | "msi_file_unreadable"
            | "msi_file_permission_denied"
            | "msi_file_hash_mismatch"
            | "msi_file_hash_unavailable" => (
                RecoveryIssueKind::DiskDrift,
                Some(RecoveryActionGroup::FileRestore),
            ),
            "missing_msi_inventory_snapshot"
            | "msi_inventory_unreadable"
            | "pkgdb_unreadable"
            | "incomplete_package_journal"
            | "unreadable_package_journal"
            | "malformed_package_journal" => (RecoveryIssueKind::RecoveryTrailMissing, None),
            "orphan_package_journal" => (
                RecoveryIssueKind::IncompleteInstall,
                Some(RecoveryActionGroup::JournalReplay),
            ),
            "stale_package_journal" | "trailing_package_journal" => (
                RecoveryIssueKind::Conflict,
                Some(RecoveryActionGroup::JournalReplay),
            ),
            _ => return None,
        };

        Some(Self {
            error_code: diagnosis.error_code.clone(),
            issue_kind,
            action_group,
            description: diagnosis.description.clone(),
            severity: diagnosis.severity,
        })
    }
}

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery_findings: Vec<RecoveryFinding>,
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
