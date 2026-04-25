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
    use crate::core::paths::{ResolvedPaths, resolved_paths};
    use crate::database;
    use crate::models::domains::install::EngineKind;
    use crate::models::domains::install::InstallerType;
    use crate::models::domains::installed::PackageStatus;
    use crate::models::domains::inventory::{
        MsiComponentRecord, MsiInventoryReceipt, MsiInventorySnapshot, MsiRegistryRecord,
        MsiShortcutRecord,
    };
    use crate::models::domains::reporting::{RecoveryActionGroup, RecoveryIssueKind};
    use crate::models::domains::shared::HashAlgorithm;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::{TempDir, tempdir};

    fn sample_snapshot(
        name: &str,
        install_dir: &std::path::Path,
        hash_hex: &str,
    ) -> MsiInventorySnapshot {
        let install_dir = install_dir
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();

        MsiInventorySnapshot {
            receipt: MsiInventoryReceipt {
                package_name: name.to_string(),
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: Some("{22222222-2222-2222-2222-222222222222}".to_string()),
                scope: winbrew_models::domains::install::InstallScope::Installed,
            },
            files: vec![MsiFileRecord {
                package_name: name.to_string(),
                path: format!("{install_dir}/bin/demo.exe"),
                normalized_path: format!("{install_dir}/bin/demo.exe"),
                hash_algorithm: Some(HashAlgorithm::Sha256),
                hash_hex: Some(hash_hex.to_string()),
                is_config_file: false,
            }],
            registry_entries: vec![MsiRegistryRecord {
                package_name: name.to_string(),
                hive: "HKLM".to_string(),
                key_path: "Software\\Demo".to_string(),
                normalized_key_path: "software\\demo".to_string(),
                value_name: "InstallPath".to_string(),
                value_data: Some(install_dir.clone()),
                previous_value: None,
            }],
            shortcuts: vec![MsiShortcutRecord {
                package_name: name.to_string(),
                path: format!("{install_dir}/Desktop/Demo.lnk"),
                normalized_path: format!("{install_dir}/desktop/demo.lnk"),
                target_path: Some(format!("{install_dir}/bin/demo.exe")),
                normalized_target_path: Some(format!("{install_dir}/bin/demo.exe")),
            }],
            components: vec![MsiComponentRecord {
                package_name: name.to_string(),
                component_id: "COMPONENT-DEMO".to_string(),
                path: Some(format!("{install_dir}/bin/demo.exe")),
                normalized_path: Some(format!("{install_dir}/bin/demo.exe")),
            }],
        }
    }

    struct TestEnvironment {
        _root: TempDir,
        paths: ResolvedPaths,
    }

    impl TestEnvironment {
        fn new() -> Self {
            let root = tempdir().expect("temp dir should be created");
            let paths = Self::build_paths(root.path());

            Self { _root: root, paths }
        }

        fn with_storage() -> Self {
            let env = Self::new();
            database::init(&env.paths).expect("database should initialize");
            env
        }

        fn build_paths(root: &Path) -> ResolvedPaths {
            let packages = root.join("packages").to_string_lossy().into_owned();
            let data = root.join("data").to_string_lossy().into_owned();
            let logs = root.join("logs").to_string_lossy().into_owned();
            let cache = root.join("cache").to_string_lossy().into_owned();

            resolved_paths(root, &packages, &data, &logs, &cache)
        }

        fn packages_root(&self) -> &Path {
            &self.paths.packages
        }

        fn create_dir(&self, path: &Path) {
            fs::create_dir_all(path).expect("directory should be created");
        }

        fn write_file(&self, path: &Path, content: &[u8]) {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent directory should be created");
            }

            fs::write(path, content).expect("file should be written");
        }

        fn db_conn(&self) -> database::DbConnection {
            database::get_conn().expect("database connection")
        }

        fn insert_package(&self, package: &InstalledPackage) -> database::DbConnection {
            let conn = self.db_conn();
            database::insert_package(&conn, package).expect("insert package");
            conn
        }

        fn make_msi_package(&self, name: &str) -> (InstalledPackage, PathBuf) {
            let install_dir = self.packages_root().join(name);
            (sample_package(name, &install_dir), install_dir)
        }

        fn make_msi_snapshot(
            &self,
            name: &str,
            install_dir: &Path,
            hash_hex: &str,
        ) -> MsiInventorySnapshot {
            sample_snapshot(name, install_dir, hash_hex)
        }
    }

    fn assert_normalized_recovery_target_path(
        finding: &crate::models::domains::reporting::RecoveryFinding,
        expected_path: &Path,
    ) {
        let expected_path = expected_path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();

        assert_eq!(finding.target_path.as_deref(), Some(expected_path.as_str()));
    }

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

    #[test]
    fn scan_msi_inventory_detects_hash_mismatch() {
        let env = TestEnvironment::with_storage();

        let (package, install_dir) = env.make_msi_package("Contoso.Msi");
        let file_path = install_dir.join("bin").join("demo.exe");
        env.create_dir(file_path.parent().expect("file parent"));
        env.write_file(&file_path, b"abc");

        let snapshot = env.make_msi_snapshot(
            "Contoso.Msi",
            &install_dir,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );

        let mut conn = env.insert_package(&package);
        database::replace_snapshot(&mut conn, &snapshot).expect("replace snapshot");

        let scan = scan_msi_inventory(&conn, &[package]);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.diagnostics[0].error_code, "msi_file_hash_mismatch");
        assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Error);
        assert!(scan.diagnostics[0].description.contains("Contoso.Msi"));

        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::DiskDrift
        );
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::FileRestore)
        );
        assert_normalized_recovery_target_path(&scan.recovery_findings[0], &file_path);
    }

    #[test]
    fn scan_msi_inventory_detects_missing_files() {
        let env = TestEnvironment::with_storage();

        let (package, install_dir) = env.make_msi_package("Contoso.Msi");
        env.create_dir(&install_dir);

        let snapshot = env.make_msi_snapshot(
            "Contoso.Msi",
            &install_dir,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );

        let mut conn = env.insert_package(&package);
        database::replace_snapshot(&mut conn, &snapshot).expect("replace snapshot");

        let scan = scan_msi_inventory(&conn, &[package]);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.diagnostics[0].error_code, "missing_msi_file");
        assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Error);
        assert!(scan.diagnostics[0].description.contains("Contoso.Msi"));

        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::DiskDrift
        );
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::FileRestore)
        );
        assert_normalized_recovery_target_path(
            &scan.recovery_findings[0],
            &install_dir.join("bin").join("demo.exe"),
        );
    }
}
