use anyhow::Result;

use crate::database;
use crate::models::Package;
use crate::models::diagnostics::{DiagnosisResult, DiagnosisSeverity};
use indicatif::ProgressBar;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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

fn check_package(pkg: &Package) -> Option<DiagnosisResult> {
    let install_dir = PathBuf::from(&pkg.install_dir);
    let install_dir = install_dir.as_path();

    if !install_dir.exists() {
        return Some(diagnosis_result(
            "missing_install_directory",
            format!(
                "{}: missing install directory ({})",
                pkg.name, pkg.install_dir
            ),
            DiagnosisSeverity::Error,
        ));
    }

    if !install_dir.is_dir() {
        return Some(diagnosis_result(
            "install_directory_not_a_directory",
            format!(
                "{}: install path is not a directory ({})",
                pkg.name, pkg.install_dir
            ),
            DiagnosisSeverity::Error,
        ));
    }

    if std::fs::read_dir(install_dir).is_err() {
        return Some(diagnosis_result(
            "install_directory_unreadable",
            format!(
                "{}: install directory is unreadable ({})",
                pkg.name, pkg.install_dir
            ),
            DiagnosisSeverity::Error,
        ));
    }

    None
}

pub fn scan_packages(packages: &[Package]) -> Vec<DiagnosisResult> {
    scan_packages_with_progress(packages, &ProgressBar::hidden())
}

pub fn scan_packages_with_progress(
    packages: &[Package],
    progress: &ProgressBar,
) -> Vec<DiagnosisResult> {
    progress.set_length(packages.len() as u64);
    progress.set_message("Scanning packages");

    let mut diagnoses: Vec<_> = packages
        .par_iter()
        .filter_map(|pkg| {
            let diagnosis = check_package(pkg);
            progress.inc(1);
            diagnosis
        })
        .collect();

    diagnoses.sort_by(|left, right| {
        left.error_code
            .cmp(&right.error_code)
            .then_with(|| left.description.cmp(&right.description))
    });
    diagnoses
}

pub fn scan_orphaned_install_dirs(
    packages_root: &Path,
    packages: &[Package],
) -> Vec<DiagnosisResult> {
    let known_packages: HashSet<_> = packages.iter().map(|pkg| pkg.name.as_str()).collect();

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

    let mut diagnoses = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

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

    diagnoses.sort_by(|left, right| {
        left.error_code
            .cmp(&right.error_code)
            .then_with(|| left.description.cmp(&right.description))
    });
    diagnoses
}

pub fn installed_packages() -> Result<Vec<Package>> {
    let conn = database::get_conn()?;
    database::list_packages(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{InstallerType, PackageStatus};
    use indicatif::ProgressBar;
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
    fn scan_packages_with_progress_sorts_diagnoses_by_error_code() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let valid_dir = temp_dir.path().join("valid");
        std::fs::create_dir_all(&valid_dir).expect("valid dir should be created");

        let packages = vec![
            sample_package("Zeta.Missing", &temp_dir.path().join("missing-zeta")),
            sample_package("Alpha.Valid", &valid_dir),
            sample_package("Beta.Missing", &temp_dir.path().join("missing-beta")),
        ];

        let diagnoses = scan_packages_with_progress(&packages, &ProgressBar::hidden());

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
