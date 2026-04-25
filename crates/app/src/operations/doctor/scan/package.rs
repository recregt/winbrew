use std::io::ErrorKind;
use std::path::Path;

use anyhow::Result;

use crate::database;
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{DiagnosisResult, DiagnosisSeverity};

use super::{ScanResult, sort_diagnoses, sort_recovery_findings};

pub(crate) type PackageInstallScan = ScanResult;

/// Validate the install path string stored on a package record.
///
/// The doctor scan treats empty paths and paths containing a null byte as
/// immediate configuration errors because they cannot represent a valid
/// filesystem location on Windows.
fn validate_install_path(pkg: &InstalledPackage) -> Option<DiagnosisResult> {
    if pkg.install_dir.trim().is_empty() {
        return Some(DiagnosisResult {
            error_code: "empty_install_path".to_string(),
            description: format!("{}: empty install directory", pkg.name),
            severity: DiagnosisSeverity::Error,
        });
    }

    if pkg.install_dir.contains('\0') {
        return Some(DiagnosisResult {
            error_code: "invalid_path_null_byte".to_string(),
            description: format!(
                "{}: path contains null byte ({})",
                pkg.name, pkg.install_dir
            ),
            severity: DiagnosisSeverity::Error,
        });
    }

    None
}

/// Translate a filesystem metadata failure into a user-facing diagnosis.
///
/// The error code depends on the error kind so the final report can distinguish
/// missing directories, permission problems, and generic unreadable paths.
fn diagnose_install_dir_error(pkg: &InstalledPackage, err: std::io::Error) -> DiagnosisResult {
    let (error_code, description) = match err.kind() {
        ErrorKind::NotFound => (
            "missing_install_directory",
            format!(
                "{}: missing install directory ({})",
                pkg.name, pkg.install_dir
            ),
        ),
        ErrorKind::PermissionDenied => (
            "install_directory_permission_denied",
            format!(
                "{}: install directory permission denied ({})",
                pkg.name, pkg.install_dir
            ),
        ),
        _ => (
            "install_directory_unreadable",
            format!(
                "{}: install directory is unreadable ({}) - {}",
                pkg.name, pkg.install_dir, err
            ),
        ),
    };

    DiagnosisResult {
        error_code: error_code.to_string(),
        description,
        severity: DiagnosisSeverity::Error,
    }
}

/// Check a single installed package for install-directory problems.
///
/// The scan is intentionally metadata-only: it validates the stored path string,
/// checks that the directory exists, and confirms that the path is actually a
/// directory. Anything else is turned into a diagnosis instead of a hard error.
pub(super) fn check_package(pkg: &InstalledPackage) -> Option<DiagnosisResult> {
    if let Some(diagnosis) = validate_install_path(pkg) {
        return Some(diagnosis);
    }

    let install_dir = Path::new(&pkg.install_dir);

    let metadata = match std::fs::metadata(install_dir) {
        Ok(metadata) => metadata,
        Err(err) => return Some(diagnose_install_dir_error(pkg, err)),
    };

    if !metadata.is_dir() {
        return Some(DiagnosisResult {
            error_code: "install_directory_not_a_directory".to_string(),
            description: format!(
                "{}: install path is not a directory ({})",
                pkg.name, pkg.install_dir
            ),
            severity: DiagnosisSeverity::Error,
        });
    }

    None
}

pub(crate) fn scan_packages(packages: &[InstalledPackage]) -> PackageInstallScan {
    let mut scan: PackageInstallScan = Default::default();

    for pkg in packages {
        if let Some(diagnosis) = check_package(pkg) {
            scan.push(diagnosis, Some(Path::new(&pkg.install_dir)));
        }
    }

    sort_diagnoses(&mut scan.diagnostics);
    sort_recovery_findings(&mut scan.recovery_findings);

    scan
}

pub(crate) fn installed_packages(
    conn: &crate::database::DbConnection,
) -> Result<Vec<InstalledPackage>> {
    database::list_packages(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domains::install::InstallerType;
    use crate::models::domains::installed::PackageStatus;
    use crate::models::domains::reporting::{RecoveryActionGroup, RecoveryIssueKind};
    use std::path::Path;
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
    fn check_package_detects_missing_directory() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let missing_dir = temp_dir.path().join("missing");
        let package = sample_package("Contoso.Missing", &missing_dir);

        let diagnosis = check_package(&package).expect("missing dir should diagnose");

        assert_eq!(diagnosis.error_code, "missing_install_directory");
        assert_eq!(diagnosis.severity, DiagnosisSeverity::Error);
        assert!(diagnosis.description.contains("Contoso.Missing"));
    }

    #[test]
    fn check_package_detects_non_directory_path() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let file_path = temp_dir.path().join("not-a-dir.txt");
        std::fs::write(&file_path, b"binary").expect("file should be created");
        let package = sample_package("Contoso.File", &file_path);

        let diagnosis = check_package(&package).expect("file path should diagnose");

        assert_eq!(diagnosis.error_code, "install_directory_not_a_directory");
        assert_eq!(diagnosis.severity, DiagnosisSeverity::Error);
        assert!(diagnosis.description.contains("Contoso.File"));
    }

    #[test]
    fn check_package_rejects_empty_install_path() {
        let package = sample_package("Contoso.Empty", Path::new(""));

        let diagnosis = check_package(&package).expect("empty path should diagnose");

        assert_eq!(diagnosis.error_code, "empty_install_path");
        assert_eq!(diagnosis.severity, DiagnosisSeverity::Error);
    }

    #[test]
    fn diagnose_install_dir_error_maps_permission_denied() {
        let package = sample_package("Contoso.Denied", Path::new("C:/deny"));
        let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);

        let diagnosis = diagnose_install_dir_error(&package, error);

        assert_eq!(diagnosis.error_code, "install_directory_permission_denied");
        assert_eq!(diagnosis.severity, DiagnosisSeverity::Error);
        assert!(diagnosis.description.contains("Contoso.Denied"));
    }

    #[test]
    fn scan_packages_attaches_reinstall_targets() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let missing_dir = temp_dir.path().join("missing");
        let package = sample_package("Contoso.Missing", &missing_dir);
        let missing_dir_string = missing_dir.to_string_lossy().to_string();

        let scan = scan_packages(&[package]);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::Reinstall)
        );
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::DiskDrift
        );
        assert_eq!(
            scan.recovery_findings[0].target_path.as_deref(),
            Some(missing_dir_string.as_str())
        );
    }

    #[test]
    fn scan_packages_sorts_diagnoses_by_error_code() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let missing_dir = temp_dir.path().join("Missing.Dir");
        let file_path = temp_dir.path().join("not-a-dir.txt");
        std::fs::write(&file_path, b"binary").expect("file should be created");

        let packages = vec![
            sample_package("Contoso.Missing", &missing_dir),
            sample_package("Contoso.File", &file_path),
            sample_package("Contoso.Empty", Path::new("")),
        ];

        let scan = scan_packages(&packages);

        assert_eq!(scan.diagnostics.len(), 3);
        assert_eq!(scan.diagnostics[0].error_code, "empty_install_path");
        assert_eq!(
            scan.diagnostics[1].error_code,
            "install_directory_not_a_directory"
        );
        assert_eq!(scan.diagnostics[2].error_code, "missing_install_directory");
        assert_eq!(scan.recovery_findings.len(), 2);
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::Reinstall)
        );
        assert_eq!(
            scan.recovery_findings[1].action_group,
            Some(RecoveryActionGroup::Reinstall)
        );
        assert_eq!(
            scan.recovery_findings[0].error_code,
            "install_directory_not_a_directory"
        );
        assert_eq!(
            scan.recovery_findings[1].error_code,
            "missing_install_directory"
        );
    }
}
