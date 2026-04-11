use anyhow::Result;

use crate::models::{DiagnosisResult, DiagnosisSeverity, Package};
use crate::storage::database;
use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

fn diagnosis_result(
    error_code: &str,
    description: String,
    severity: DiagnosisSeverity,
) -> DiagnosisResult {
    DiagnosisResult {
        error_code: error_code.to_string(),
        description,
        severity,
    }
}

fn validate_install_path(pkg: &Package) -> Option<DiagnosisResult> {
    if pkg.install_dir.trim().is_empty() {
        return Some(diagnosis_result(
            "empty_install_path",
            format!("{}: empty install directory", pkg.name),
            DiagnosisSeverity::Error,
        ));
    }

    if pkg.install_dir.contains('\0') {
        return Some(diagnosis_result(
            "invalid_path_null_byte",
            format!(
                "{}: path contains null byte ({})",
                pkg.name, pkg.install_dir
            ),
            DiagnosisSeverity::Error,
        ));
    }

    None
}

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

    diagnosis_result(error_code, description, DiagnosisSeverity::Error)
}

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
        return Some(diagnosis_result(
            "install_directory_not_a_directory",
            format!(
                "{}: install path is not a directory ({})",
                pkg.name, pkg.install_dir
            ),
            DiagnosisSeverity::Error,
        ));
    }

    None
}

/// Scans installed packages and optionally advances a progress bar while doing so.
///
/// When a progress bar is supplied, the iterator uses indicatif's Rayon integration
/// so progress updates stay synchronized with the parallel scan.
pub(super) fn scan_packages_with_progress(
    packages: &[Package],
    progress: Option<&ProgressBar>,
) -> Vec<DiagnosisResult> {
    let diagnoses: Vec<DiagnosisResult> = match progress {
        Some(progress) => {
            progress.set_length(packages.len() as u64);
            progress.set_message("Scanning packages");

            let diagnoses = packages
                .par_iter()
                .progress_with(progress.clone())
                .filter_map(check_package)
                .collect();

            progress.finish_and_clear();
            diagnoses
        }
        None => packages.par_iter().filter_map(check_package).collect(),
    };

    sort_diagnoses(diagnoses)
}

/// Scans the package root for directories that do not correspond to installed packages.
pub(super) fn scan_orphaned_install_dirs(
    packages_root: &Path,
    packages: &[Package],
) -> Vec<DiagnosisResult> {
    let mut known_packages = HashSet::with_capacity(packages.len());
    known_packages.extend(packages.iter().map(|pkg| pkg.name.as_str()));

    let entries = match fs::read_dir(packages_root) {
        Ok(entries) => entries,
        Err(err) => {
            return vec![diagnosis_result(
                "packages_root_unreadable",
                format!(
                    "packages root: unreadable packages directory ({}) - {err}",
                    packages_root.to_string_lossy()
                ),
                DiagnosisSeverity::Error,
            )];
        }
    };

    let estimated_orphans = (packages.len().saturating_div(10)).max(8);
    let mut diagnoses = Vec::with_capacity(estimated_orphans);

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

        diagnoses.push(diagnosis_result(
            "orphan_install_directory",
            format!(
                "{}: orphan install directory ({})",
                package_name,
                path.to_string_lossy()
            ),
            DiagnosisSeverity::Warning,
        ));
    }

    sort_diagnoses(diagnoses)
}

/// Loads the current installed package inventory from the database.
pub(super) fn installed_packages() -> Result<Vec<Package>> {
    let conn = database::get_conn()?;
    database::list_packages(&conn)
}

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
    fn scan_packages_with_progress_sorts_diagnoses_by_error_code() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let valid_dir = temp_dir.path().join("valid");
        std::fs::create_dir_all(&valid_dir).expect("valid dir should be created");

        let packages = vec![
            sample_package("Zeta.Missing", &temp_dir.path().join("missing-zeta")),
            sample_package("Alpha.Valid", &valid_dir),
            sample_package("Beta.Missing", &temp_dir.path().join("missing-beta")),
        ];

        let diagnoses = scan_packages_with_progress(&packages, None);

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
