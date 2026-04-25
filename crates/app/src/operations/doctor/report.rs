//! Summary assembly for the doctor report.
//!
//! This module turns the raw diagnostics produced by `scan` into the final
//! [`crate::models::domains::reporting::HealthReport`]. It is responsible for path rendering,
//! diagnostic ordering, fallback diagnostics when package inventory lookup
//! fails, and the final error count used by the UI.

use anyhow::Result;
use std::path::Path;
use std::time::Instant;

use crate::AppContext;
use crate::database;

use super::scan;
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{
    DiagnosisResult, DiagnosisSeverity, HealthReport, RecoveryFinding,
};

/// Convert a path into the display string used in the final report.
///
/// The conversion is lossy on purpose so the report stays printable even if a
/// path contains non-UTF-8 bytes.
fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().into_owned()
}

/// Order diagnostics for the final report.
///
/// This is intentionally different from `scan::sort_diagnoses`: scan modules
/// keep their own source-local ordering, while the final report groups all
/// collected diagnostics by severity first so the UI reads from errors to
/// warnings.
fn sort_report_diagnostics(left: &DiagnosisResult, right: &DiagnosisResult) -> std::cmp::Ordering {
    left.severity
        .cmp(&right.severity)
        .then_with(|| left.error_code.cmp(&right.error_code))
        .then_with(|| left.description.cmp(&right.description))
}

/// Load installed packages or convert the failure into a diagnostic entry.
///
/// A database lookup failure should not prevent the doctor report from being
/// generated. Instead, the function returns an empty package list plus a single
/// error diagnostic that explains why package inventory is unavailable.
fn collect_packages(
    packages_result: Result<Vec<InstalledPackage>>,
) -> (Vec<InstalledPackage>, Vec<DiagnosisResult>) {
    match packages_result {
        Ok(packages) => (packages, Vec::new()),
        Err(err) => (
            Vec::new(),
            vec![DiagnosisResult {
                error_code: "installed_packages_unavailable".to_string(),
                description: format!("installed packages: unavailable ({err})"),
                severity: DiagnosisSeverity::Error,
            }],
        ),
    }
}

/// Collect recovery findings from the initial package-loading diagnostics.
fn collect_initial_recovery_findings(diagnostics: &[DiagnosisResult]) -> Vec<RecoveryFinding> {
    diagnostics
        .iter()
        .filter_map(RecoveryFinding::from_diagnosis)
        .collect()
}

fn sort_recovery_findings(left: &RecoveryFinding, right: &RecoveryFinding) -> std::cmp::Ordering {
    left.action_group
        .cmp(&right.action_group)
        .then_with(|| left.severity.cmp(&right.severity))
        .then_with(|| left.error_code.cmp(&right.error_code))
        .then_with(|| left.target_path.cmp(&right.target_path))
        .then_with(|| left.description.cmp(&right.description))
}

/// Build a full health report for the current application context.
///
/// The function snapshots the current paths, collects installed packages,
/// scans package directories, journal recovery data, and MSI inventory, then
/// sorts the resulting diagnostics and computes a final error count. The
/// returned report is intentionally pre-rendered with display-friendly paths
/// so the caller can present it directly.
pub fn health_report(ctx: &AppContext) -> Result<HealthReport> {
    let paths = &ctx.paths;
    let started_at = Instant::now();
    let conn = database::get_conn()?;

    let (packages, mut diagnostics) = collect_packages(scan::installed_packages(&conn));
    let mut recovery_findings = collect_initial_recovery_findings(&diagnostics);
    let orphan_scan = scan::scan_orphaned_install_dirs(&paths.packages, &packages);
    let scan::PackageInstallScan {
        diagnostics: package_diagnostics,
        recovery_findings: package_recovery_findings,
    } = scan::scan_packages(&packages);
    let scan::MsiInventoryScan {
        diagnostics: msi_diagnostics,
        recovery_findings: msi_recovery_findings,
    } = scan::scan_msi_inventory(&conn, &packages);
    let scan::PackageJournalScan {
        diagnostics: journal_diagnostics,
        recovery_findings: journal_recovery_findings,
    } = scan::scan_package_journals(paths, &packages);

    diagnostics.extend(package_diagnostics);
    diagnostics.extend(msi_diagnostics);
    diagnostics.extend(orphan_scan.diagnostics);
    diagnostics.extend(journal_diagnostics);
    // Merge every scan source first, then sort the final report once.
    diagnostics.sort_unstable_by(sort_report_diagnostics);
    recovery_findings.extend(package_recovery_findings);
    recovery_findings.extend(msi_recovery_findings);
    recovery_findings.extend(orphan_scan.recovery_findings);
    recovery_findings.extend(journal_recovery_findings);
    recovery_findings.sort_unstable_by(sort_recovery_findings);

    let error_count = diagnostics
        .iter()
        .filter(|diagnosis| matches!(diagnosis.severity, DiagnosisSeverity::Error))
        .count();

    Ok(HealthReport {
        database_path: display_path(&paths.db),
        database_exists: paths.db.exists(),
        catalog_database_path: display_path(&paths.catalog_db),
        catalog_database_exists: paths.catalog_db.exists(),
        install_root_source: if ctx.root_from_env {
            "env override".to_string()
        } else {
            "config:paths.root".to_string()
        },
        install_root: display_path(&paths.root),
        install_root_exists: paths.root.exists(),
        packages_dir: display_path(&paths.packages),
        diagnostics,
        recovery_findings,
        scan_duration: started_at.elapsed(),
        error_count,
    })
}

#[cfg(test)]
mod tests {
    use super::{collect_initial_recovery_findings, collect_packages, sort_report_diagnostics};
    use crate::models::domains::reporting::{
        DiagnosisResult, DiagnosisSeverity, RecoveryActionGroup, RecoveryIssueKind,
    };
    use anyhow::anyhow;

    #[test]
    fn collect_packages_converts_errors_into_diagnostics() {
        let (packages, diagnostics) = collect_packages(Err(anyhow!("database unavailable")));

        assert!(packages.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].error_code, "installed_packages_unavailable");
        assert_eq!(diagnostics[0].severity, DiagnosisSeverity::Error);
        assert!(diagnostics[0].description.contains("database unavailable"));
    }

    #[test]
    fn sort_report_diagnostics_keeps_errors_before_warnings() {
        let mut diagnostics = [
            DiagnosisResult {
                error_code: "warning_b".to_string(),
                description: "warning".to_string(),
                severity: DiagnosisSeverity::Warning,
            },
            DiagnosisResult {
                error_code: "error_a".to_string(),
                description: "error".to_string(),
                severity: DiagnosisSeverity::Error,
            },
            DiagnosisResult {
                error_code: "error_c".to_string(),
                description: "error".to_string(),
                severity: DiagnosisSeverity::Error,
            },
        ];

        diagnostics.sort_unstable_by(sort_report_diagnostics);

        assert_eq!(diagnostics[0].severity, DiagnosisSeverity::Error);
        assert_eq!(diagnostics[1].severity, DiagnosisSeverity::Error);
        assert_eq!(diagnostics[2].severity, DiagnosisSeverity::Warning);
    }

    #[test]
    fn collect_packages_keeps_empty_package_lists_empty() {
        let (packages, diagnostics) = collect_packages(Ok(Vec::new()));

        assert!(packages.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn collect_initial_recovery_findings_maps_policy_issues() {
        let diagnostics = vec![DiagnosisResult {
            error_code: "stale_package_journal".to_string(),
            description: "Contoso.App: recovery journal does not match installed package"
                .to_string(),
            severity: DiagnosisSeverity::Error,
        }];

        let findings = collect_initial_recovery_findings(&diagnostics);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].issue_kind, RecoveryIssueKind::Conflict);
        assert_eq!(
            findings[0].action_group,
            Some(RecoveryActionGroup::JournalReplay)
        );
        assert_eq!(findings[0].severity, DiagnosisSeverity::Error);
    }
}
