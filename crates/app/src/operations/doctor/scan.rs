//! Package and filesystem scanning for doctor diagnostics.
//!
//! The scan phase inspects installed package records, their directories, and
//! persisted MSI inventory data. It intentionally uses lightweight filesystem
//! metadata checks so the doctor command can diagnose common state problems
//! without launching or probing package binaries.
//!
//! Two categories of diagnostics are produced here:
//!
//! - broken package records whose install paths are missing, invalid, or unreadable
//! - MSI inventory files that are missing or no longer match their stored hashes
//! - orphaned directories under the package root that no longer have a database record

use anyhow::Result;

use crate::core::hash::hash_file;
use crate::core::{HashError, verify_hash};
use crate::models::{
    DiagnosisResult, DiagnosisSeverity, EngineKind, Package, RecoveryActionGroup, RecoveryFinding,
    RecoveryIssueKind,
};
use crate::storage::database;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub(super) struct PackageJournalScan {
    pub diagnostics: Vec<DiagnosisResult>,
    pub recovery_findings: Vec<RecoveryFinding>,
}

impl PackageJournalScan {
    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            recovery_findings: Vec::new(),
        }
    }

    fn push(&mut self, diagnosis: DiagnosisResult, target_path: Option<&Path>) {
        if let Some(finding) = RecoveryFinding::from_diagnosis(&diagnosis) {
            let finding = match target_path {
                Some(target_path) => finding.with_target_path(target_path.to_string_lossy()),
                None => finding,
            };
            self.recovery_findings.push(finding);
        }

        self.diagnostics.push(diagnosis);
    }
}

pub(super) struct OrphanInstallScan {
    pub diagnostics: Vec<DiagnosisResult>,
    pub recovery_findings: Vec<RecoveryFinding>,
}

impl OrphanInstallScan {
    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            recovery_findings: Vec::new(),
        }
    }

    fn push(&mut self, package_name: &str, path: &Path) {
        let description = format!(
            "{}: orphan install directory ({})",
            package_name,
            path.to_string_lossy()
        );

        self.diagnostics.push(DiagnosisResult {
            error_code: "orphan_install_directory".to_string(),
            description: description.clone(),
            severity: DiagnosisSeverity::Warning,
        });

        self.recovery_findings.push(RecoveryFinding {
            error_code: "orphan_install_directory".to_string(),
            issue_kind: RecoveryIssueKind::IncompleteInstall,
            action_group: Some(RecoveryActionGroup::OrphanCleanup),
            description,
            severity: DiagnosisSeverity::Warning,
            target_path: Some(path.to_string_lossy().into_owned()),
        });
    }
}

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

/// Scan persisted MSI inventory data and report files that no longer match.
pub(super) fn scan_msi_inventory(
    conn: &crate::storage::DbConnection,
    packages: &[Package],
) -> Vec<DiagnosisResult> {
    let mut diagnoses = Vec::new();

    for pkg in packages
        .iter()
        .filter(|pkg| matches!(pkg.engine_kind, EngineKind::Msi))
    {
        let snapshot = match database::get_snapshot(conn, &pkg.name) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                diagnoses.push(DiagnosisResult {
                    error_code: "missing_msi_inventory_snapshot".to_string(),
                    description: format!("{}: MSI inventory snapshot is missing", pkg.name),
                    severity: DiagnosisSeverity::Error,
                });
                continue;
            }
            Err(err) => {
                diagnoses.push(DiagnosisResult {
                    error_code: "msi_inventory_unreadable".to_string(),
                    description: format!("{}: MSI inventory is unreadable - {err}", pkg.name),
                    severity: DiagnosisSeverity::Error,
                });
                continue;
            }
        };

        for file in &snapshot.files {
            if let Some(diagnosis) = diagnose_msi_file(pkg, file) {
                diagnoses.push(diagnosis);
            }
        }
    }

    sort_diagnoses(diagnoses)
}

/// Scan package journal files under `data/pkgdb` and report recovery issues.
pub(super) fn scan_package_journals(root: &Path, packages: &[Package]) -> PackageJournalScan {
    let pkgdb_root = crate::core::pkgdb_dir_at(root);

    if !pkgdb_root.exists() {
        return PackageJournalScan::new();
    }

    let package_lookup: HashMap<&str, &Package> = packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();

    let entries = match fs::read_dir(&pkgdb_root) {
        Ok(entries) => entries,
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                return PackageJournalScan::new();
            }

            let mut result = PackageJournalScan::new();
            result.push(
                DiagnosisResult {
                    error_code: "pkgdb_unreadable".to_string(),
                    description: format!(
                        "pkgdb root: unreadable journal directory ({}) - {err}",
                        pkgdb_root.to_string_lossy()
                    ),
                    severity: DiagnosisSeverity::Error,
                },
                None,
            );
            return result;
        }
    };

    let mut result = PackageJournalScan::new();

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };

        if !file_type.is_dir() {
            continue;
        }

        let journal_path = entry.path().join("journal.jsonl");
        if !journal_path.exists() {
            continue;
        }

        match database::JournalReader::read_committed(&journal_path) {
            Ok(entries) => {
                for diagnosis in
                    diagnose_committed_journal(&journal_path, &entries, &package_lookup)
                {
                    result.push(diagnosis, Some(&journal_path));
                }
            }
            Err(database::JournalReadError::Incomplete { .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "incomplete_package_journal".to_string(),
                        description: format!(
                            "{}: incomplete recovery journal",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
            }
            Err(database::JournalReadError::Read { .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "unreadable_package_journal".to_string(),
                        description: format!(
                            "{}: recovery journal is unreadable",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
            }
            Err(database::JournalReadError::MalformedLine { line, .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "malformed_package_journal".to_string(),
                        description: format!(
                            "{}: recovery journal has malformed line {line}",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
            }
            Err(database::JournalReadError::TrailingEntries { line, .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "trailing_package_journal".to_string(),
                        description: format!(
                            "{}: recovery journal has trailing entries after commit on line {line}",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    Some(&journal_path),
                );
            }
        }
    }

    result.diagnostics = sort_diagnoses(result.diagnostics);
    result
        .recovery_findings
        .sort_unstable_by(sort_recovery_findings);

    result
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
) -> OrphanInstallScan {
    let mut known_packages = HashSet::with_capacity(packages.len());
    known_packages.extend(packages.iter().map(|pkg| pkg.name.as_str()));

    let entries = match fs::read_dir(packages_root) {
        Ok(entries) => entries,
        Err(err) => {
            let mut result = OrphanInstallScan::new();
            result.diagnostics.push(DiagnosisResult {
                error_code: "packages_root_unreadable".to_string(),
                description: format!(
                    "packages root: unreadable packages directory ({}) - {err}",
                    packages_root.to_string_lossy()
                ),
                severity: DiagnosisSeverity::Error,
            });
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

        result.push(package_name, &path);
    }

    result.diagnostics = sort_diagnoses(result.diagnostics);
    result
        .recovery_findings
        .sort_unstable_by(sort_recovery_findings);

    result
}

/// Load the current installed package inventory from the database.
///
/// The caller owns the shared database connection so package scanning and MSI
/// inventory scanning can reuse the same snapshot of the database state.
pub(super) fn installed_packages(conn: &crate::storage::DbConnection) -> Result<Vec<Package>> {
    database::list_packages(conn)
}

fn diagnose_msi_file(
    pkg: &Package,
    file: &crate::models::MsiFileRecord,
) -> Option<DiagnosisResult> {
    let path = Path::new(&file.path);

    let metadata = match fs::metadata(path) {
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

fn diagnose_committed_journal(
    journal_path: &Path,
    entries: &[database::JournalEntry],
    packages: &HashMap<&str, &Package>,
) -> Vec<DiagnosisResult> {
    let Some((package_id, version, engine, install_dir)) =
        entries.iter().find_map(|entry| match entry {
            database::JournalEntry::Metadata {
                package_id,
                version,
                engine,
                install_dir,
                dependencies: _,
                engine_metadata: _,
            } => Some((
                package_id.as_str(),
                version.as_str(),
                engine.as_str(),
                install_dir.as_str(),
            )),
            _ => None,
        })
    else {
        return vec![DiagnosisResult {
            error_code: "missing_journal_metadata".to_string(),
            description: format!(
                "{}: committed recovery journal is missing metadata",
                journal_path.to_string_lossy()
            ),
            severity: DiagnosisSeverity::Error,
        }];
    };

    let Some(package) = packages.get(package_id) else {
        return vec![DiagnosisResult {
            error_code: "orphan_package_journal".to_string(),
            description: format!(
                "{}: committed recovery journal has no installed package",
                journal_path.to_string_lossy()
            ),
            severity: DiagnosisSeverity::Warning,
        }];
    };

    if package.version != version
        || !package.engine_kind.as_str().eq_ignore_ascii_case(engine)
        || package.install_dir != install_dir
    {
        return vec![DiagnosisResult {
            error_code: "stale_package_journal".to_string(),
            description: format!(
                "{}: recovery journal does not match installed package {} ({})",
                journal_path.to_string_lossy(),
                package.name,
                package.version
            ),
            severity: DiagnosisSeverity::Warning,
        }];
    }

    Vec::new()
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

fn sort_recovery_findings(left: &RecoveryFinding, right: &RecoveryFinding) -> std::cmp::Ordering {
    left.severity
        .cmp(&right.severity)
        .then_with(|| left.error_code.cmp(&right.error_code))
        .then_with(|| left.target_path.cmp(&right.target_path))
        .then_with(|| left.description.cmp(&right.description))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::resolved_paths;
    use crate::models::{InstallerType, PackageStatus, RecoveryActionGroup, RecoveryIssueKind};
    use crate::storage;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;
    use winbrew_models::{
        EngineKind, MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
        MsiRegistryRecord, MsiShortcutRecord,
    };

    fn sample_package(name: &str, install_dir: &std::path::Path) -> Package {
        Package {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            engine_kind: InstallerType::Portable.into(),
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().into_owned(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    fn sample_msi_package(name: &str, install_dir: &std::path::Path) -> Package {
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

    fn init_storage(root: &Path) {
        let packages = root.join("packages").to_string_lossy().into_owned();
        let data = root.join("data").to_string_lossy().into_owned();
        let logs = root.join("logs").to_string_lossy().into_owned();
        let cache = root.join("cache").to_string_lossy().into_owned();
        let paths = resolved_paths(root, &packages, &data, &logs, &cache);

        storage::init(&paths).expect("storage should initialize");
    }

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
                scope: winbrew_models::InstallScope::Installed,
            },
            files: vec![MsiFileRecord {
                package_name: name.to_string(),
                path: format!("{install_dir}/bin/demo.exe"),
                normalized_path: format!("{install_dir}/bin/demo.exe"),
                hash_algorithm: Some(winbrew_models::HashAlgorithm::Sha256),
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

        let scan = scan_orphaned_install_dirs(&packages_root, &packages);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.diagnostics[0].error_code, "orphan_install_directory");
        assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Warning);
        assert!(scan.diagnostics[0].description.contains("Contoso.Orphan"));
        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::OrphanCleanup)
        );
        assert_eq!(
            scan.recovery_findings[0].target_path.as_deref(),
            Some(orphan_dir.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn scan_msi_inventory_detects_hash_mismatch() {
        let root = tempdir().expect("temp root");
        init_storage(root.path());

        let install_dir = root.path().join("packages").join("Contoso.Msi");
        let file_path = install_dir.join("bin").join("demo.exe");
        fs::create_dir_all(file_path.parent().expect("file parent"))
            .expect("install dir should be created");
        fs::write(&file_path, b"abc").expect("payload should be written");

        let package = sample_msi_package("Contoso.Msi", &install_dir);
        let snapshot = sample_snapshot(
            "Contoso.Msi",
            &install_dir,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );

        let mut conn = storage::get_conn().expect("database connection");
        storage::insert_package(&conn, &package).expect("insert package");
        storage::replace_snapshot(&mut conn, &snapshot).expect("replace snapshot");

        let diagnoses = scan_msi_inventory(&conn, &[package]);

        assert_eq!(diagnoses.len(), 1);
        assert_eq!(diagnoses[0].error_code, "msi_file_hash_mismatch");
        assert_eq!(diagnoses[0].severity, DiagnosisSeverity::Error);
        assert!(diagnoses[0].description.contains("Contoso.Msi"));
    }

    #[test]
    fn scan_msi_inventory_detects_missing_files() {
        let root = tempdir().expect("temp root");
        init_storage(root.path());

        let install_dir = root.path().join("packages").join("Contoso.Msi");
        fs::create_dir_all(&install_dir).expect("install dir should be created");

        let package = sample_msi_package("Contoso.Msi", &install_dir);
        let snapshot = sample_snapshot(
            "Contoso.Msi",
            &install_dir,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );

        let mut conn = storage::get_conn().expect("database connection");
        storage::insert_package(&conn, &package).expect("insert package");
        storage::replace_snapshot(&mut conn, &snapshot).expect("replace snapshot");

        let diagnoses = scan_msi_inventory(&conn, &[package]);

        assert_eq!(diagnoses.len(), 1);
        assert_eq!(diagnoses[0].error_code, "missing_msi_file");
        assert_eq!(diagnoses[0].severity, DiagnosisSeverity::Error);
        assert!(diagnoses[0].description.contains("Contoso.Msi"));
    }

    #[test]
    fn scan_package_journals_detects_incomplete_journal() {
        let root = tempdir().expect("temp root");

        let mut writer =
            database::JournalWriter::open_for_package(root.path(), "Contoso.Recover", "1.0.0")
                .expect("open journal");
        writer
            .append(&database::JournalEntry::Metadata {
                package_id: "Contoso.Recover".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
                install_dir: r"C:\winbrew\apps\Contoso.Recover".to_string(),
                dependencies: Vec::new(),
                engine_metadata: None,
            })
            .expect("write metadata");
        writer.flush().expect("flush journal");

        let scan = scan_package_journals(root.path(), &[]);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.diagnostics[0].error_code, "incomplete_package_journal");
        assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Error);
        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::RecoveryTrailMissing
        );
        assert!(scan.recovery_findings[0].target_path.is_none());
    }

    #[test]
    fn scan_package_journals_detects_orphan_committed_journal() {
        let root = tempdir().expect("temp root");

        let mut writer =
            database::JournalWriter::open_for_package(root.path(), "Contoso.Orphan", "1.0.0")
                .expect("open journal");
        writer
            .append(&database::JournalEntry::Metadata {
                package_id: "Contoso.Orphan".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
                install_dir: r"C:\winbrew\apps\Contoso.Orphan".to_string(),
                dependencies: Vec::new(),
                engine_metadata: None,
            })
            .expect("write metadata");
        writer
            .append(&database::JournalEntry::Commit {
                installed_at: "2026-04-12T00:00:00Z".to_string(),
            })
            .expect("write commit");
        writer.flush().expect("flush journal");
        let journal_path = writer.path().to_path_buf();

        let scan = scan_package_journals(root.path(), &[]);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.diagnostics[0].error_code, "orphan_package_journal");
        assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Warning);
        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::IncompleteInstall
        );
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::JournalReplay)
        );
        assert_eq!(
            scan.recovery_findings[0].target_path.as_deref(),
            Some(journal_path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn scan_package_journals_tracks_trailing_journal_replay_target() {
        let root = tempdir().expect("temp root");

        let mut writer =
            database::JournalWriter::open_for_package(root.path(), "Contoso.Trailing", "1.0.0")
                .expect("open journal");
        writer
            .append(&database::JournalEntry::Metadata {
                package_id: "Contoso.Trailing".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
                install_dir: r"C:\winbrew\apps\Contoso.Trailing".to_string(),
                dependencies: Vec::new(),
                engine_metadata: None,
            })
            .expect("write metadata");
        writer
            .append(&database::JournalEntry::Commit {
                installed_at: "2026-04-12T00:00:00Z".to_string(),
            })
            .expect("write commit");
        writer
            .append(&database::JournalEntry::FsCreate {
                path: r"C:\winbrew\apps\Contoso.Trailing\payload.exe".to_string(),
                hash: None,
            })
            .expect("write trailing entry");
        writer.flush().expect("flush journal");
        let journal_path = writer.path().to_path_buf();

        let scan = scan_package_journals(root.path(), &[]);

        assert_eq!(scan.diagnostics.len(), 1);
        assert_eq!(scan.diagnostics[0].error_code, "trailing_package_journal");
        assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Error);
        assert_eq!(scan.recovery_findings.len(), 1);
        assert_eq!(
            scan.recovery_findings[0].issue_kind,
            RecoveryIssueKind::Conflict
        );
        assert_eq!(
            scan.recovery_findings[0].action_group,
            Some(RecoveryActionGroup::JournalReplay)
        );
        assert_eq!(
            scan.recovery_findings[0].target_path.as_deref(),
            Some(journal_path.to_string_lossy().as_ref())
        );
    }
}
