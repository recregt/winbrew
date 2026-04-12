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
fn diagnose_msi_file_error(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EngineKind, InstallerType, PackageStatus};
    use tempfile::tempdir;

    fn sample_package(name: &str, install_dir: &std::path::Path) -> Package {
        Package {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Msi,
            engine_kind: EngineKind::Msi,
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().into_owned(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn diagnose_msi_file_error_maps_missing_and_permission_denied() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let package = sample_package("Contoso.Msi", temp_dir.path());
        let file = crate::models::MsiFileRecord {
            package_name: "Contoso.Msi".to_string(),
            path: temp_dir
                .path()
                .join("missing.exe")
                .to_string_lossy()
                .into_owned(),
            normalized_path: temp_dir
                .path()
                .join("missing.exe")
                .to_string_lossy()
                .into_owned(),
            hash_algorithm: Some(winbrew_models::HashAlgorithm::Sha256),
            hash_hex: Some("00".repeat(32)),
            is_config_file: false,
        };

        let not_found = diagnose_msi_file_error(
            &package,
            &file,
            std::io::Error::from(std::io::ErrorKind::NotFound),
        );
        let denied = diagnose_msi_file_error(
            &package,
            &file,
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );

        assert_eq!(not_found.error_code, "missing_msi_file");
        assert_eq!(denied.error_code, "msi_file_permission_denied");
        assert_eq!(not_found.severity, crate::models::DiagnosisSeverity::Error);
        assert_eq!(denied.severity, crate::models::DiagnosisSeverity::Error);
        assert!(not_found.description.contains("Contoso.Msi"));
        assert!(denied.description.contains("Contoso.Msi"));
    }
}
