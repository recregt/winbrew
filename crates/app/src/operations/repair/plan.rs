use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::models::domains::reporting::{HealthReport, RecoveryActionGroup};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRestorePackage {
    pub name: String,
    pub target_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairPlan {
    pub journal_paths: Vec<PathBuf>,
    pub orphan_paths: Vec<PathBuf>,
    pub file_restore_packages: Vec<FileRestorePackage>,
    pub reinstall_packages: Vec<String>,
    pub file_restore_count: usize,
    pub reinstall_count: usize,
}

impl RepairPlan {
    pub fn is_empty(&self) -> bool {
        self.journal_paths.is_empty()
            && self.orphan_paths.is_empty()
            && self.file_restore_packages.is_empty()
            && self.reinstall_packages.is_empty()
    }
}

/// Build the grouped recovery plan from a health report.
pub fn build_repair_plan(report: &HealthReport, packages_root: &Path) -> RepairPlan {
    let journal_paths = recovery_paths(report, RecoveryActionGroup::JournalReplay);
    let orphan_paths = recovery_paths(report, RecoveryActionGroup::OrphanCleanup);
    let file_restore_packages =
        recovery_file_restore_packages(report, packages_root, RecoveryActionGroup::FileRestore);
    let mut reinstall_packages =
        recovery_package_names(report, packages_root, RecoveryActionGroup::Reinstall);
    reinstall_packages.retain(|package_name| {
        !file_restore_packages
            .iter()
            .any(|candidate| candidate.name == *package_name)
    });

    RepairPlan {
        journal_paths,
        orphan_paths,
        file_restore_packages,
        reinstall_packages,
        file_restore_count: recovery_count(report, RecoveryActionGroup::FileRestore),
        reinstall_count: recovery_count(report, RecoveryActionGroup::Reinstall),
    }
}

fn recovery_paths(report: &HealthReport, action_group: RecoveryActionGroup) -> Vec<PathBuf> {
    let mut paths = report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
        .filter_map(|finding| finding.target_path.as_ref().map(PathBuf::from))
        .collect::<Vec<_>>();

    paths.sort();
    paths.dedup();
    paths
}

fn recovery_count(report: &HealthReport, action_group: RecoveryActionGroup) -> usize {
    report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
        .count()
}

fn recovery_package_names(
    report: &HealthReport,
    packages_root: &Path,
    action_group: RecoveryActionGroup,
) -> Vec<String> {
    let mut package_names = report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
        .filter_map(|finding| {
            finding.target_path.as_deref().and_then(|target_path| {
                package_name_from_target_path(packages_root, Path::new(target_path))
            })
        })
        .collect::<Vec<_>>();

    package_names.sort_unstable();
    package_names.dedup();
    package_names
}

fn recovery_file_restore_packages(
    report: &HealthReport,
    packages_root: &Path,
    action_group: RecoveryActionGroup,
) -> Vec<FileRestorePackage> {
    let mut package_targets = BTreeMap::<String, Vec<PathBuf>>::new();

    for finding in report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
    {
        let Some(target_path) = finding.target_path.as_deref() else {
            continue;
        };

        let Some(package_name) =
            package_name_from_target_path(packages_root, Path::new(target_path))
        else {
            continue;
        };

        package_targets
            .entry(package_name)
            .or_default()
            .push(PathBuf::from(target_path));
    }

    package_targets
        .into_iter()
        .map(|(name, mut target_paths)| {
            target_paths.sort();
            target_paths.dedup();

            FileRestorePackage { name, target_paths }
        })
        .collect()
}

fn package_name_from_target_path(packages_root: &Path, target_path: &Path) -> Option<String> {
    let relative_path = target_path.strip_prefix(packages_root).ok()?;
    let package_name = relative_path.components().next()?.as_os_str().to_str()?;

    if package_name.is_empty() {
        return None;
    }

    Some(package_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domains::reporting::{
        DiagnosisSeverity, HealthReport, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind,
    };
    use crate::models::reporting::HealthScanTimings;
    use std::path::Path;

    #[test]
    fn build_repair_plan_groups_targets_and_counts_findings() {
        let report = HealthReport {
            database_path: "db.sqlite".to_string(),
            database_exists: true,
            catalog_database_path: "catalog.sqlite".to_string(),
            catalog_database_exists: true,
            install_root_source: "config".to_string(),
            install_root: "C:/Tools".to_string(),
            install_root_exists: true,
            packages_dir: "C:/Tools/packages".to_string(),
            diagnostics: Vec::new(),
            recovery_findings: vec![
                RecoveryFinding {
                    error_code: "missing_install_directory".to_string(),
                    issue_kind: RecoveryIssueKind::DiskDrift,
                    action_group: Some(RecoveryActionGroup::Reinstall),
                    description: "pkg reinstall".to_string(),
                    severity: DiagnosisSeverity::Error,
                    target_path: Some("C:/Tools/packages/Contoso.App".to_string()),
                },
                RecoveryFinding {
                    error_code: "missing_msi_file".to_string(),
                    issue_kind: RecoveryIssueKind::DiskDrift,
                    action_group: Some(RecoveryActionGroup::FileRestore),
                    description: "pkg file".to_string(),
                    severity: DiagnosisSeverity::Error,
                    target_path: Some("C:/Tools/packages/Contoso.App/bin/tool.exe".to_string()),
                },
            ],
            scan_timings: HealthScanTimings::default(),
            scan_duration: std::time::Duration::from_millis(1),
            error_count: 2,
        };

        let plan = build_repair_plan(&report, Path::new("C:/Tools/packages"));

        assert!(plan.journal_paths.is_empty());
        assert!(plan.orphan_paths.is_empty());
        assert!(plan.reinstall_packages.is_empty());
        assert_eq!(plan.file_restore_packages.len(), 1);
        assert_eq!(plan.file_restore_count, 1);
        assert_eq!(plan.reinstall_count, 1);
    }
}
