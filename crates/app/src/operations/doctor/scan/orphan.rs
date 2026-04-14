use std::collections::HashSet;
use std::path::Path;

use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{DiagnosisResult, DiagnosisSeverity};

use super::{OrphanInstallScan, sort_diagnoses, sort_recovery_findings};

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
            let mut result = OrphanInstallScan::new();
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

    let mut result = OrphanInstallScan::new();

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

    result.diagnostics = sort_diagnoses(result.diagnostics);
    result
        .recovery_findings
        .sort_unstable_by(sort_recovery_findings);

    result
}
