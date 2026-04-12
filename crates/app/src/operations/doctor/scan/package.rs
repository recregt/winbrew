use std::io::ErrorKind;
use std::path::Path;

use crate::models::{DiagnosisResult, DiagnosisSeverity, Package};

/// Validate the install path string stored on a package record.
///
/// The doctor scan treats empty paths and paths containing a null byte as
/// immediate configuration errors because they cannot represent a valid
/// filesystem location on Windows.
pub(super) fn validate_install_path(pkg: &Package) -> Option<DiagnosisResult> {
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
pub(super) fn diagnose_install_dir_error(pkg: &Package, err: std::io::Error) -> DiagnosisResult {
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
pub(super) fn check_package(pkg: &Package) -> Option<DiagnosisResult> {
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
