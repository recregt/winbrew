use anyhow::Result;

use crate::models::{
    DiagnosisResult, DiagnosisSeverity, EngineKind, Package, RecoveryActionGroup, RecoveryFinding,
    RecoveryIssueKind,
};
use crate::storage::database;

mod journal;
mod msi;
mod orphan;
mod package;

pub(crate) struct PackageJournalScan {
    pub diagnostics: Vec<DiagnosisResult>,
    pub recovery_findings: Vec<RecoveryFinding>,
}

impl PackageJournalScan {
    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            recovery_findings: Vec::new(),
        }
    }

    fn push(&mut self, diagnosis: DiagnosisResult, target_path: Option<&std::path::Path>) {
        if let Some(finding) = RecoveryFinding::from_diagnosis(&diagnosis) {
            let finding = match target_path {
                Some(target_path) => finding.with_target_path(target_path.to_string_lossy()),
                None => finding,
            };
            self.recovery_findings.push(finding);
        }

        self.diagnostics.push(diagnosis);
    }
}

pub(crate) struct OrphanInstallScan {
    pub diagnostics: Vec<DiagnosisResult>,
    pub recovery_findings: Vec<RecoveryFinding>,
}

impl OrphanInstallScan {
    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            recovery_findings: Vec::new(),
        }
    }

    fn push(&mut self, package_name: &str, path: &std::path::Path) {
        let description = format!(
            "{}: orphan install directory ({})",
            package_name,
            path.to_string_lossy()
        );

        self.diagnostics.push(DiagnosisResult {
            error_code: "orphan_install_directory".to_string(),
            description: description.clone(),
            severity: DiagnosisSeverity::Warning,
        });

        self.recovery_findings.push(RecoveryFinding {
            error_code: "orphan_install_directory".to_string(),
            issue_kind: RecoveryIssueKind::IncompleteInstall,
            action_group: Some(RecoveryActionGroup::OrphanCleanup),
            description,
            severity: DiagnosisSeverity::Warning,
            target_path: Some(path.to_string_lossy().into_owned()),
        });
    }
}

pub(crate) fn check_package(pkg: &Package) -> Option<DiagnosisResult> {
    package::check_package(pkg)
}

#[allow(dead_code)]
pub(crate) fn diagnose_install_dir_error(pkg: &Package, err: std::io::Error) -> DiagnosisResult {
    package::diagnose_install_dir_error(pkg, err)
}

#[allow(dead_code)]
pub(crate) fn validate_install_path(pkg: &Package) -> Option<DiagnosisResult> {
    package::validate_install_path(pkg)
}

pub(crate) fn diagnose_msi_file(
    pkg: &Package,
    file: &crate::models::MsiFileRecord,
) -> Option<DiagnosisResult> {
    msi::diagnose_msi_file(pkg, file)
}

#[allow(dead_code)]
pub(crate) fn diagnose_msi_file_error(
    pkg: &Package,
    file: &crate::models::MsiFileRecord,
    err: std::io::Error,
) -> DiagnosisResult {
    msi::diagnose_msi_file_error(pkg, file, err)
}

pub(crate) fn scan_package_journals(
    root: &std::path::Path,
    packages: &[Package],
) -> PackageJournalScan {
    journal::scan_package_journals(root, packages)
}

pub(crate) fn scan_orphaned_install_dirs(
    packages_root: &std::path::Path,
    packages: &[Package],
) -> OrphanInstallScan {
    orphan::scan_orphaned_install_dirs(packages_root, packages)
}

/// Scan installed packages and return diagnostics for broken install roots.
pub(crate) fn scan_packages(packages: &[Package]) -> Vec<DiagnosisResult> {
    sort_diagnoses(packages.iter().filter_map(check_package).collect())
}

/// Scan persisted MSI inventory data and report files that no longer match.
pub(crate) fn scan_msi_inventory(
    conn: &crate::storage::DbConnection,
    packages: &[Package],
) -> Vec<DiagnosisResult> {
    let mut diagnoses = Vec::new();

    for pkg in packages
        .iter()
        .filter(|pkg| matches!(pkg.engine_kind, EngineKind::Msi))
    {
        let snapshot = match database::get_snapshot(conn, &pkg.name) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                diagnoses.push(DiagnosisResult {
                    error_code: "missing_msi_inventory_snapshot".to_string(),
                    description: format!("{}: MSI inventory snapshot is missing", pkg.name),
                    severity: DiagnosisSeverity::Error,
                });
                continue;
            }
            Err(err) => {
                diagnoses.push(DiagnosisResult {
                    error_code: "msi_inventory_unreadable".to_string(),
                    description: format!("{}: MSI inventory is unreadable - {err}", pkg.name),
                    severity: DiagnosisSeverity::Error,
                });
                continue;
            }
        };

        for file in &snapshot.files {
            if let Some(diagnosis) = diagnose_msi_file(pkg, file) {
                diagnoses.push(diagnosis);
            }
        }
    }

    sort_diagnoses(diagnoses)
}

/// Load the current installed package inventory from the database.
///
/// The caller owns the shared database connection so package scanning and MSI
/// inventory scanning can reuse the same snapshot of the database state.
pub(crate) fn installed_packages(conn: &crate::storage::DbConnection) -> Result<Vec<Package>> {
    database::list_packages(conn)
}

/// Sort diagnostics deterministically by code and description.
pub(super) fn sort_diagnoses(mut diagnoses: Vec<DiagnosisResult>) -> Vec<DiagnosisResult> {
    diagnoses.sort_unstable_by(|left, right| {
        left.error_code
            .cmp(&right.error_code)
            .then_with(|| left.description.cmp(&right.description))
    });
    diagnoses
}

pub(super) fn sort_recovery_findings(
    left: &RecoveryFinding,
    right: &RecoveryFinding,
) -> std::cmp::Ordering {
    left.severity
        .cmp(&right.severity)
        .then_with(|| left.error_code.cmp(&right.error_code))
        .then_with(|| left.target_path.cmp(&right.target_path))
        .then_with(|| left.description.cmp(&right.description))
}

#[cfg(test)]
mod tests;
