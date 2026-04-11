//! Package and filesystem scanning for doctor diagnostics.
//!
//! The scan phase inspects installed package records and the directories they
//! point at. It intentionally uses lightweight filesystem metadata checks so
//! the doctor command can diagnose common state problems without launching or
//! probing package binaries.
//!
//! Two categories of diagnostics are produced here:
//!
//! - broken package records whose install paths are missing, invalid, or unreadable
//! - orphaned directories under the package root that no longer have a database record

use anyhow::Result;

use crate::models::{DiagnosisResult, DiagnosisSeverity, Package};
use crate::storage::database;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// Validate the install path string stored on a package record.
///
/// The doctor scan treats empty paths and paths containing a null byte as
/// immediate configuration errors because they cannot represent a valid
/// filesystem location on Windows.
fn validate_install_path(pkg: &Package) -> Option<DiagnosisResult> {
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
fn diagnose_install_dir_error(pkg: &Package, err: std::io::Error) -> DiagnosisResult {
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
fn check_package(pkg: &Package) -> Option<DiagnosisResult> {
    if let Some(diagnosis) = validate_install_path(pkg) {
        return Some(diagnosis);
    }

    let install_dir = Path::new(&pkg.install_dir);

    let metadata = match fs::metadata(install_dir) {
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

/// Scan installed packages and return diagnostics for broken install roots.
pub(super) fn scan_packages(packages: &[Package]) -> Vec<DiagnosisResult> {
    sort_diagnoses(packages.iter().filter_map(check_package).collect())
}

/// Scan the package root for directories that do not correspond to packages in the database.
///
/// Orphaned directories are reported as warnings because they indicate stale
/// filesystem state rather than a broken package record. If the root directory
/// itself cannot be read, the function returns a single error diagnostic so the
/// caller can surface the storage problem directly.
pub(super) fn scan_orphaned_install_dirs(
    packages_root: &Path,
    packages: &[Package],
) -> Vec<DiagnosisResult> {
    let mut known_packages = HashSet::with_capacity(packages.len());
    known_packages.extend(packages.iter().map(|pkg| pkg.name.as_str()));

    let entries = match fs::read_dir(packages_root) {
        Ok(entries) => entries,
        Err(err) => {
            return vec![DiagnosisResult {
                error_code: "packages_root_unreadable".to_string(),
                description: format!(
                    "packages root: unreadable packages directory ({}) - {err}",
                    packages_root.to_string_lossy()
                ),
                severity: DiagnosisSeverity::Error,
            }];
        }
    };

    let mut diagnoses = Vec::new();

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

        diagnoses.push(DiagnosisResult {
            error_code: "orphan_install_directory".to_string(),
            description: format!(
                "{}: orphan install directory ({})",
                package_name,
                path.to_string_lossy()
            ),
            severity: DiagnosisSeverity::Warning,
        });
    }

    sort_diagnoses(diagnoses)
}

/// Load the current installed package inventory from the database.
///
/// The scan layer owns the database access here so the caller can treat the
/// package inventory as just another input to report generation.
pub(super) fn installed_packages() -> Result<Vec<Package>> {
    let conn = database::get_conn()?;
    database::list_packages(&conn)
}

/// Sort diagnostics deterministically by code and description.
fn sort_diagnoses(mut diagnoses: Vec<DiagnosisResult>) -> Vec<DiagnosisResult> {
    diagnoses.sort_unstable_by(|left, right| {
        left.error_code
            .cmp(&right.error_code)
            .then_with(|| left.description.cmp(&right.description))
    });
    diagnoses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{InstallerType, PackageStatus};
    use tempfile::tempdir;

    fn sample_package(name: &str, install_dir: &std::path::Path) -> Package {
        Package {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            engine_kind: InstallerType::Portable.into(),
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().into_owned(),
            msix_package_full_name: None,
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn diagnosis_result_check_package_detects_missing_directory() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let missing_dir = temp_dir.path().join("missing");
        let package = sample_package("Contoso.Missing", &missing_dir);

        let diagnosis = check_package(&package).expect("missing dir should diagnose");

        assert_eq!(diagnosis.error_code, "missing_install_directory");
        assert_eq!(diagnosis.severity, DiagnosisSeverity::Error);
        assert!(diagnosis.description.contains("Contoso.Missing"));
    }

    #[test]
    fn diagnosis_result_check_package_detects_non_directory_path() {
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
    fn diagnosis_result_check_package_rejects_empty_install_path() {
        let package = sample_package("Contoso.Empty", Path::new(""));

        let diagnosis = check_package(&package).expect("empty path should diagnose");

        assert_eq!(diagnosis.error_code, "empty_install_path");
        assert_eq!(diagnosis.severity, DiagnosisSeverity::Error);
    }

    #[test]
    fn diagnose_install_dir_error_maps_permission_denied() {
        let package = sample_package("Contoso.Denied", Path::new("C:/deny"));
        let error = std::io::Error::from(ErrorKind::PermissionDenied);

        let diagnosis = diagnose_install_dir_error(&package, error);

        assert_eq!(diagnosis.error_code, "install_directory_permission_denied");
        assert_eq!(diagnosis.severity, DiagnosisSeverity::Error);
        assert!(diagnosis.description.contains("Contoso.Denied"));
    }

    #[test]
    fn scan_packages_sorts_diagnoses_by_error_code() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let valid_dir = temp_dir.path().join("valid");
        std::fs::create_dir_all(&valid_dir).expect("valid dir should be created");

        let packages = vec![
            sample_package("Zeta.Missing", &temp_dir.path().join("missing-zeta")),
            sample_package("Alpha.Valid", &valid_dir),
            sample_package("Beta.Missing", &temp_dir.path().join("missing-beta")),
        ];

        let diagnoses = scan_packages(&packages);

        assert_eq!(diagnoses.len(), 2);
        assert_eq!(diagnoses[0].error_code, "missing_install_directory");
        assert_eq!(diagnoses[1].error_code, "missing_install_directory");
    }

    #[test]
    fn scan_orphaned_install_dirs_detects_directories_without_packages() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let packages_root = temp_dir.path().join("packages");
        std::fs::create_dir_all(&packages_root).expect("packages root should be created");

        let orphan_dir = packages_root.join("Contoso.Orphan");
        std::fs::create_dir_all(&orphan_dir).expect("orphan dir should be created");

        let packages = vec![sample_package(
            "Contoso.Known",
            &packages_root.join("Contoso.Known"),
        )];

        let diagnoses = scan_orphaned_install_dirs(&packages_root, &packages);

        assert_eq!(diagnoses.len(), 1);
        assert_eq!(diagnoses[0].error_code, "orphan_install_directory");
        assert_eq!(diagnoses[0].severity, DiagnosisSeverity::Warning);
        assert!(diagnoses[0].description.contains("Contoso.Orphan"));
    }
}
