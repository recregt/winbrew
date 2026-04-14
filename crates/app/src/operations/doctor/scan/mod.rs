//! Shared coordination for doctor scan sources.
//!
//! Each submodule owns a single scan source, while this module provides the
//! shared result container, sorting helpers, and the entry points used by the
//! doctor workflow.

use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{
    DiagnosisResult, DiagnosisSeverity, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind,
};

mod journal;
mod msi;
mod orphan;
mod package;

pub(super) use msi::{MsiInventoryScan, scan_msi_inventory};
pub(super) use package::{PackageInstallScan, installed_packages, scan_packages};

#[derive(Debug, Default)]
pub(super) struct ScanResult {
    pub(super) diagnostics: Vec<DiagnosisResult>,
    pub(super) recovery_findings: Vec<RecoveryFinding>,
}

impl ScanResult {
    fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, diagnosis: DiagnosisResult, target_path: Option<&std::path::Path>) {
        if let Some(finding) = RecoveryFinding::from_diagnosis(&diagnosis) {
            let finding = match target_path {
                Some(target_path) => {
                    finding.with_target_path(target_path.to_string_lossy().into_owned())
                }
                None => finding,
            };
            self.recovery_findings.push(finding);
        }

        self.diagnostics.push(diagnosis);
    }

    fn push_orphan(&mut self, package_name: &str, path: &std::path::Path) {
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

pub(super) type PackageJournalScan = ScanResult;
pub(super) type OrphanInstallScan = ScanResult;

pub(super) fn scan_package_journals(
    paths: &crate::core::paths::ResolvedPaths,
    packages: &[InstalledPackage],
) -> PackageJournalScan {
    journal::scan_package_journals(paths, packages)
}

pub(super) fn scan_orphaned_install_dirs(
    packages_root: &std::path::Path,
    packages: &[InstalledPackage],
) -> OrphanInstallScan {
    orphan::scan_orphaned_install_dirs(packages_root, packages)
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
