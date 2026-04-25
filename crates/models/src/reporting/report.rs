//! Health and recovery report models.

use serde::{Deserialize, Serialize, Serializer};
use std::time::Duration;

use super::diagnostics::DiagnosisResult;

/// Recovery issue buckets used to classify doctor findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryIssueKind {
    /// The repair trail or journal was missing.
    RecoveryTrailMissing,
    /// The install is incomplete.
    IncompleteInstall,
    /// A recovery conflict exists between stored state and the filesystem.
    Conflict,
    /// The filesystem content drifted from recorded expectations.
    DiskDrift,
}

/// Recovery action group used to drive repair UI choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryActionGroup {
    /// Replay a committed journal.
    JournalReplay,
    /// Remove orphan directories.
    OrphanCleanup,
    /// Restore individual files.
    FileRestore,
    /// Reinstall the package.
    Reinstall,
}

/// A recovery-oriented interpretation of a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryFinding {
    /// Stable diagnostic code mirrored from the originating diagnostic.
    pub error_code: String,
    /// High-level issue classification.
    pub issue_kind: RecoveryIssueKind,
    /// Optional user-facing action grouping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_group: Option<RecoveryActionGroup>,
    /// Human-readable description of the recovery issue.
    pub description: String,
    /// Severity inherited from the originating diagnostic.
    pub severity: super::diagnostics::DiagnosisSeverity,
    /// Optional target path for repair actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
}

impl RecoveryFinding {
    /// Map a diagnostic into a recovery finding when the error code is actionable.
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
            | "malformed_package_journal"
            | "missing_journal_metadata" => (RecoveryIssueKind::RecoveryTrailMissing, None),
            "orphan_install_directory" => (
                RecoveryIssueKind::IncompleteInstall,
                Some(RecoveryActionGroup::OrphanCleanup),
            ),
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
            target_path: None,
        })
    }

    /// Attach a filesystem target path to the finding.
    pub fn with_target_path(mut self, target_path: impl Into<String>) -> Self {
        self.target_path = Some(target_path.into());
        self
    }
}

/// Timing breakdown for the doctor scan pipeline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct HealthScanTimings {
    /// Time spent opening the database connection.
    #[serde(serialize_with = "serialize_duration_millis")]
    pub database_connection: Duration,
    /// Time spent loading installed packages.
    #[serde(serialize_with = "serialize_duration_millis")]
    pub installed_packages: Duration,
    /// Time spent validating package install directories.
    #[serde(serialize_with = "serialize_duration_millis")]
    pub package_scan: Duration,
    /// Time spent validating MSI inventory snapshots and files.
    #[serde(serialize_with = "serialize_duration_millis")]
    pub msi_scan: Duration,
    /// Time spent checking for orphaned package directories.
    #[serde(serialize_with = "serialize_duration_millis")]
    pub orphan_scan: Duration,
    /// Time spent scanning committed package journals.
    #[serde(serialize_with = "serialize_duration_millis")]
    pub journal_scan: Duration,
}

/// The full health report emitted by the doctor workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthReport {
    /// Filesystem path to the Winbrew database.
    pub database_path: String,
    /// Whether the database path exists.
    pub database_exists: bool,
    /// Filesystem path to the catalog database.
    pub catalog_database_path: String,
    /// Whether the catalog database path exists.
    pub catalog_database_exists: bool,
    /// Where the install root came from in configuration resolution.
    pub install_root_source: String,
    /// Filesystem path to the install root.
    pub install_root: String,
    /// Whether the install root exists.
    pub install_root_exists: bool,
    /// Display path for the packages directory.
    pub packages_dir: String,
    /// Sorted diagnostics collected during the scan.
    pub diagnostics: Vec<DiagnosisResult>,
    /// Recovery findings derived from the diagnostics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery_findings: Vec<RecoveryFinding>,
    /// Timing breakdown for the scan pipeline.
    pub scan_timings: HealthScanTimings,
    /// Total scan duration.
    #[serde(serialize_with = "serialize_duration_millis")]
    pub scan_duration: Duration,
    /// Count of diagnostics with error severity.
    pub error_count: usize,
}

/// Runtime report sections rendered by the info command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    /// Ordered report sections.
    pub sections: Vec<ReportSection>,
}

/// A titled section of key/value runtime report data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSection {
    /// Section title.
    pub title: String,
    /// Key/value entries in display order.
    pub entries: Vec<(String, String)>,
}

impl RuntimeReport {
    /// Build a runtime report from pre-computed sections.
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
