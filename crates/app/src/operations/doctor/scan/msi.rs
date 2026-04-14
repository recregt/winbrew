use std::io::ErrorKind;
use std::path::Path;

use crate::core::hash::hash_file;
use crate::core::{HashError, verify_hash};
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::inventory::MsiFileRecord;
use crate::models::domains::reporting::{DiagnosisResult, DiagnosisSeverity, RecoveryFinding};

use super::{sort_diagnoses, sort_recovery_findings};

pub(crate) struct MsiInventoryScan {
    pub(crate) diagnostics: Vec<DiagnosisResult>,
    pub(crate) recovery_findings: Vec<RecoveryFinding>,
}

impl MsiInventoryScan {
    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            recovery_findings: Vec::new(),
        }
    }

    fn push(&mut self, diagnosis: DiagnosisResult, target_path: Option<&Path>) {
        if let Some(finding) = RecoveryFinding::from_diagnosis(&diagnosis) {
            let finding = match target_path {
                Some(target_path) => {
                    finding.with_target_path(target_path.to_string_lossy().into_owned())
                }
                None => finding,
            };
            self.recovery_findings.push(finding);
        }

        self.diagnostics.push(diagnosis);
    }
}

/// Verify a single MSI file against the stored inventory snapshot.
pub(super) fn diagnose_msi_file(
    pkg: &InstalledPackage,
    file: &MsiFileRecord,
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
    pkg: &InstalledPackage,
    file: &MsiFileRecord,
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

pub(crate) fn scan_msi_inventory(
    conn: &crate::database::DbConnection,
    packages: &[InstalledPackage],
) -> MsiInventoryScan {
    let mut scan = MsiInventoryScan::new();

    for pkg in packages.iter().filter(|pkg| {
        matches!(
            pkg.engine_kind,
            crate::models::domains::install::EngineKind::Msi
        )
    }) {
        let snapshot = match crate::database::get_snapshot(conn, &pkg.name) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                scan.push(
                    DiagnosisResult {
                        error_code: "missing_msi_inventory_snapshot".to_string(),
                        description: format!("{}: MSI inventory snapshot is missing", pkg.name),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
                continue;
            }
            Err(err) => {
                scan.push(
                    DiagnosisResult {
                        error_code: "msi_inventory_unreadable".to_string(),
                        description: format!("{}: MSI inventory is unreadable - {err}", pkg.name),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
                continue;
            }
        };

        for file in &snapshot.files {
            if let Some(diagnosis) = diagnose_msi_file(pkg, file) {
                scan.push(diagnosis, Some(Path::new(&file.path)));
            }
        }
    }

    scan.diagnostics = sort_diagnoses(scan.diagnostics);
    scan.recovery_findings
        .sort_unstable_by(sort_recovery_findings);

    scan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domains::install::EngineKind;
    use crate::models::domains::install::InstallerType;
    use crate::models::domains::installed::PackageStatus;
    use crate::models::domains::reporting::{RecoveryActionGroup, RecoveryIssueKind};
    use crate::models::domains::shared::HashAlgorithm;
    use tempfile::tempdir;

    fn sample_package(name: &str, install_dir: &std::path::Path) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Msi,
            deployment_kind: InstallerType::Msi.deployment_kind(),
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
        let file = MsiFileRecord {
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
            hash_algorithm: Some(HashAlgorithm::Sha256),
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
        assert_eq!(
            not_found.severity,
            crate::models::domains::reporting::DiagnosisSeverity::Error
        );
        assert_eq!(denied.severity, DiagnosisSeverity::Error);
        assert!(not_found.description.contains("Contoso.Msi"));
        assert!(denied.description.contains("Contoso.Msi"));
    }

    #[test]
    fn scan_msi_inventory_attaches_file_restore_targets() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let package = sample_package("Contoso.Msi", temp_dir.path());
        let file = MsiFileRecord {
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
            hash_algorithm: Some(HashAlgorithm::Sha256),
            hash_hex: Some("00".repeat(32)),
            is_config_file: false,
        };

        let diagnosis = diagnose_msi_file_error(
            &package,
            &file,
            std::io::Error::from(std::io::ErrorKind::NotFound),
        );
        let mut scan = MsiInventoryScan::new();
        scan.push(diagnosis, Some(Path::new(&file.path)));

        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::FileRestore)
        );
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::DiskDrift
        );
        assert_eq!(
            scan.recovery_findings[0].target_path.as_deref(),
            Some(file.path.as_str())
        );
    }
}
