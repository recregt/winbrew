use std::collections::HashSet;
use std::path::Path;

use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{DiagnosisResult, DiagnosisSeverity};

use super::{OrphanInstallScan, ScanResult, sort_diagnoses, sort_recovery_findings};

/// Scan the package root for directories that do not correspond to packages in the database.
///
/// Orphaned directories are reported as warnings because they indicate stale
/// filesystem state rather than a broken package record. If the root directory
/// itself cannot be read, the function returns a single error diagnostic so the
/// caller can surface the database problem directly.
pub(super) fn scan_orphaned_install_dirs(
    packages_root: &Path,
    packages: &[InstalledPackage],
) -> OrphanInstallScan {
    let mut known_packages = HashSet::with_capacity(packages.len());
    known_packages.extend(packages.iter().map(|pkg| pkg.name.as_str()));

    let entries = match std::fs::read_dir(packages_root) {
        Ok(entries) => entries,
        Err(err) => {
            let mut result = ScanResult::default();
            result.push(
                DiagnosisResult {
                    error_code: "packages_root_unreadable".to_string(),
                    description: format!(
                        "packages root: unreadable packages directory ({}) - {err}",
                        packages_root.to_string_lossy()
                    ),
                    severity: DiagnosisSeverity::Error,
                },
                None,
            );
            return result;
        }
    };

    let mut result = ScanResult::default();

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };

        if !file_type.is_dir() {
            continue;
        }

        let path = entry.path();

        let package_name = match path.file_name().and_then(|value| value.to_str()) {
            Some(package_name) => package_name,
            None => continue,
        };

        if known_packages.contains(package_name) {
            continue;
        }

        result.push_orphan(package_name, &path);
    }

    sort_diagnoses(&mut result.diagnostics);
    sort_recovery_findings(&mut result.recovery_findings);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domains::install::InstallerType;
    use crate::models::domains::installed::{InstalledPackage, PackageStatus};
    use crate::models::domains::reporting::{RecoveryActionGroup, RecoveryIssueKind};
    use std::fs;
    use tempfile::tempdir;

    fn sample_package(name: &str, install_dir: &std::path::Path) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            deployment_kind: InstallerType::Portable.deployment_kind(),
            engine_kind: InstallerType::Portable.into(),
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().into_owned(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn scan_orphaned_install_dirs_detects_directories_without_packages() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let packages_root = temp_dir.path().join("packages");
        fs::create_dir_all(&packages_root).expect("packages root should be created");

        let orphan_dir = packages_root.join("Contoso.Orphan");
        fs::create_dir_all(&orphan_dir).expect("orphan directory should be created");

        let known_package = sample_package("Contoso.Known", &packages_root.join("Contoso.Known"));

        let scan = scan_orphaned_install_dirs(&packages_root, &[known_package]);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.diagnostics[0].error_code, "orphan_install_directory");
        assert_eq!(
            scan.diagnostics[0].severity,
            crate::models::domains::reporting::DiagnosisSeverity::Warning
        );
        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::IncompleteInstall
        );
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::OrphanCleanup)
        );
        assert_eq!(
            scan.recovery_findings[0].target_path.as_deref(),
            Some(orphan_dir.to_string_lossy().as_ref())
        );
    }
}
