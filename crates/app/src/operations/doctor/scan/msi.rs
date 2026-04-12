use std::io::ErrorKind;
use std::path::Path;

use crate::core::hash::hash_file;
use crate::core::{HashError, verify_hash};
use crate::models::{DiagnosisResult, DiagnosisSeverity, Package};

/// Verify a single MSI file against the stored inventory snapshot.
pub(super) fn diagnose_msi_file(
    pkg: &Package,
    file: &crate::models::MsiFileRecord,
) -> Option<DiagnosisResult> {
    let path = Path::new(&file.path);

    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            return Some(diagnose_msi_file_error(pkg, file, err));
        }
    };

    if !metadata.is_file() {
        return Some(DiagnosisResult {
            error_code: "msi_file_not_a_file".to_string(),
            description: format!("{}: MSI file path is not a file ({})", pkg.name, file.path),
            severity: DiagnosisSeverity::Error,
        });
    }

    let (Some(hash_algorithm), Some(expected_hash)) =
        (file.hash_algorithm, file.hash_hex.as_deref())
    else {
        return None;
    };

    let actual_hash = match hash_file(path, hash_algorithm) {
        Ok(actual_hash) => actual_hash,
        Err(err) => {
            return Some(DiagnosisResult {
                error_code: "msi_file_unreadable".to_string(),
                description: format!(
                    "{}: MSI file is unreadable for hashing ({}) - {}",
                    pkg.name, file.path, err
                ),
                severity: DiagnosisSeverity::Error,
            });
        }
    };

    match verify_hash(expected_hash, actual_hash) {
        Ok(()) => None,
        Err(HashError::ChecksumMismatch { expected, actual }) => Some(DiagnosisResult {
            error_code: "msi_file_hash_mismatch".to_string(),
            description: format!(
                "{}: MSI file hash mismatch for {} (expected {}, got {})",
                pkg.name, file.path, expected, actual
            ),
            severity: DiagnosisSeverity::Error,
        }),
        Err(err) => Some(DiagnosisResult {
            error_code: "msi_file_hash_unavailable".to_string(),
            description: format!(
                "{}: MSI file hash could not be verified for {} - {}",
                pkg.name, file.path, err
            ),
            severity: DiagnosisSeverity::Error,
        }),
    }
}

/// Translate a filesystem metadata failure into an MSI file diagnosis.
pub(super) fn diagnose_msi_file_error(
    pkg: &Package,
    file: &crate::models::MsiFileRecord,
    err: std::io::Error,
) -> DiagnosisResult {
    let (error_code, description) = match err.kind() {
        ErrorKind::NotFound => (
            "missing_msi_file",
            format!("{}: missing MSI file ({})", pkg.name, file.path),
        ),
        ErrorKind::PermissionDenied => (
            "msi_file_permission_denied",
            format!("{}: MSI file permission denied ({})", pkg.name, file.path),
        ),
        _ => (
            "msi_file_unreadable",
            format!(
                "{}: MSI file is unreadable ({}) - {}",
                pkg.name, file.path, err
            ),
        ),
    };

    DiagnosisResult {
        error_code: error_code.to_string(),
        description,
        severity: DiagnosisSeverity::Error,
    }
}
