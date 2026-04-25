use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;

use crate::core::paths::ResolvedPaths;
use crate::database;
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{DiagnosisResult, DiagnosisSeverity};
use crate::models::domains::shared::DeploymentKind;
use tracing::debug;

use super::{PackageJournalScan, sort_diagnoses, sort_recovery_findings};

mod error_codes {
    pub const PKGDB_UNREADABLE: &str = "pkgdb_unreadable";
    pub const INCOMPLETE_JOURNAL: &str = "incomplete_package_journal";
    pub const UNREADABLE_JOURNAL: &str = "unreadable_package_journal";
    pub const MALFORMED_JOURNAL: &str = "malformed_package_journal";
    pub const TRAILING_JOURNAL: &str = "trailing_package_journal";
    pub const MISSING_METADATA: &str = "missing_journal_metadata";
    pub const ORPHAN_JOURNAL: &str = "orphan_package_journal";
    pub const STALE_JOURNAL: &str = "stale_package_journal";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct JournalMetadata<'a> {
    pub(super) package_id: &'a str,
    pub(super) version: &'a str,
    pub(super) engine: &'a str,
    pub(super) deployment_kind: DeploymentKind,
    pub(super) install_dir: &'a str,
}

/// Create a standardized diagnosis result with consistent formatting.
///
/// This is the base builder for all diagnosis results in the journal scanner.
/// Use [`journal_error`] for journal-specific errors with path prefixing.
///
/// # Arguments
/// * `error_code` - Machine-readable error identifier.
/// * `description` - Human-readable error description.
/// * `severity` - Error severity level.
#[inline]
fn diagnosis(
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

/// Create a journal-specific diagnosis with automatic path prefixing.
///
/// Formats the description as `{path}: {message}` for consistent error
/// messages.
///
/// # Example
/// ```ignore
/// let diag = journal_error(
///     &path,
///     "incomplete_journal",
///     "incomplete recovery journal",
///     DiagnosisSeverity::Error,
/// );
/// ```
#[inline]
fn journal_error(
    journal_path: &Path,
    error_code: &str,
    message: impl std::fmt::Display,
    severity: DiagnosisSeverity,
) -> DiagnosisResult {
    diagnosis(
        error_code,
        format!("{}: {}", journal_path.to_string_lossy(), message),
        severity,
    )
}

/// Convert a [`database::JournalReadError`] into a structured diagnosis result.
///
/// Maps all journal reading error variants to appropriate diagnosis codes and
/// severity levels. Returns the diagnosis along with an optional path
/// reference for recovery findings.
///
/// # Error Mapping
/// - `Incomplete` -> error, no path reference
/// - `Read` -> error, no path reference
/// - `MalformedLine` -> error with line number, no path reference
/// - `TrailingEntries` -> error with line number, with path reference
fn journal_read_error_diagnosis(
    journal_path: &Path,
    error: database::JournalReadError,
) -> (DiagnosisResult, Option<&Path>) {
    match error {
        database::JournalReadError::Incomplete { .. } => (
            journal_error(
                journal_path,
                error_codes::INCOMPLETE_JOURNAL,
                "incomplete recovery journal",
                DiagnosisSeverity::Error,
            ),
            None,
        ),
        database::JournalReadError::Read { .. } => (
            journal_error(
                journal_path,
                error_codes::UNREADABLE_JOURNAL,
                "recovery journal is unreadable",
                DiagnosisSeverity::Error,
            ),
            None,
        ),
        database::JournalReadError::MalformedLine { line, .. } => (
            journal_error(
                journal_path,
                error_codes::MALFORMED_JOURNAL,
                format!("recovery journal has malformed line {line}"),
                DiagnosisSeverity::Error,
            ),
            None,
        ),
        database::JournalReadError::TrailingEntries { line, .. } => (
            journal_error(
                journal_path,
                error_codes::TRAILING_JOURNAL,
                format!("recovery journal has trailing entries after commit on line {line}"),
                DiagnosisSeverity::Error,
            ),
            Some(journal_path),
        ),
    }
}

pub(super) fn extract_journal_metadata(
    entries: &[database::JournalEntry],
) -> Option<JournalMetadata<'_>> {
    entries.iter().find_map(|entry| match entry {
        database::JournalEntry::Metadata {
            package_id,
            version,
            engine,
            deployment_kind,
            install_dir,
            dependencies: _,
            commands: _,
            bin: _,
            command_resolution: _,
            engine_metadata: _,
        } => Some(JournalMetadata {
            package_id: package_id.as_str(),
            version: version.as_str(),
            engine: engine.as_str(),
            deployment_kind: *deployment_kind,
            install_dir: install_dir.as_str(),
        }),
        _ => None,
    })
}

pub(super) fn journal_metadata_matches_package(
    package: &InstalledPackage,
    metadata: &JournalMetadata<'_>,
) -> bool {
    package.version == metadata.version
        && package
            .engine_kind
            .as_str()
            .eq_ignore_ascii_case(metadata.engine)
        && package.install_dir == metadata.install_dir
        && package.deployment_kind == metadata.deployment_kind
}

fn process_journal_entry(
    entry_path: &Path,
    package_lookup: &HashMap<&str, &InstalledPackage>,
    result: &mut PackageJournalScan,
) {
    let journal_path = entry_path.join("journal.jsonl");

    match database::JournalReader::read_committed(&journal_path) {
        Ok(entries) => {
            for diagnosis in diagnose_committed_journal(&journal_path, &entries, package_lookup) {
                result.push(diagnosis, Some(&journal_path));
            }
        }
        Err(database::JournalReadError::Read { source, .. })
            if source.kind() == ErrorKind::NotFound =>
        {
            debug!(path = %journal_path.display(), "missing journal file, skipping package directory");
        }
        Err(error) => {
            let (diagnosis, target_path) = journal_read_error_diagnosis(&journal_path, error);
            result.push(diagnosis, target_path);
        }
    }
}

/// Scan package journal files under `data/pkgdb` and report recovery issues.
pub(super) fn scan_package_journals(
    paths: &ResolvedPaths,
    packages: &[InstalledPackage],
) -> PackageJournalScan {
    let pkgdb_root = &paths.pkgdb;

    if !pkgdb_root.exists() {
        debug!(path = %pkgdb_root.display(), "pkgdb root does not exist, skipping journal scan");
        return PackageJournalScan::new();
    }

    let package_lookup: HashMap<&str, &InstalledPackage> = packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();

    let entries = match std::fs::read_dir(pkgdb_root) {
        Ok(entries) => entries,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                debug!(path = %pkgdb_root.display(), "pkgdb root disappeared before journal scan");
                return PackageJournalScan::new();
            }

            let mut result = PackageJournalScan::new();
            result.push(
                diagnosis(
                    error_codes::PKGDB_UNREADABLE,
                    format!(
                        "pkgdb root: unreadable journal directory ({}) - {err}",
                        pkgdb_root.to_string_lossy()
                    ),
                    DiagnosisSeverity::Error,
                ),
                None,
            );
            return result;
        }
    };

    let mut result = PackageJournalScan::new();

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                debug!(path = %pkgdb_root.display(), error = %err, "skipping unreadable pkgdb entry");
                continue;
            }
        };

        let entry_path = entry.path();

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                debug!(path = %entry_path.display(), error = %err, "skipping pkgdb entry with unreadable file type");
                continue;
            }
        };

        if !file_type.is_dir() {
            continue;
        }

        process_journal_entry(&entry_path, &package_lookup, &mut result);
    }

    result.diagnostics = sort_diagnoses(result.diagnostics);
    result
        .recovery_findings
        .sort_unstable_by(sort_recovery_findings);

    result
}

fn diagnose_committed_journal(
    journal_path: &Path,
    entries: &[database::JournalEntry],
    packages: &HashMap<&str, &InstalledPackage>,
) -> Vec<DiagnosisResult> {
    let Some(metadata) = extract_journal_metadata(entries) else {
        return vec![journal_error(
            journal_path,
            error_codes::MISSING_METADATA,
            "committed recovery journal is missing metadata",
            DiagnosisSeverity::Error,
        )];
    };

    diagnose_committed_journal_metadata(journal_path, &metadata, packages)
        .into_iter()
        .collect()
}

fn diagnose_committed_journal_metadata(
    journal_path: &Path,
    metadata: &JournalMetadata<'_>,
    packages: &HashMap<&str, &InstalledPackage>,
) -> Option<DiagnosisResult> {
    let Some(package) = packages.get(metadata.package_id) else {
        return Some(journal_error(
            journal_path,
            error_codes::ORPHAN_JOURNAL,
            "committed recovery journal has no installed package",
            DiagnosisSeverity::Warning,
        ));
    };

    if !journal_metadata_matches_package(package, metadata) {
        return Some(journal_error(
            journal_path,
            error_codes::STALE_JOURNAL,
            format!(
                "recovery journal does not match installed package {} ({})",
                package.name, package.version
            ),
            DiagnosisSeverity::Warning,
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        JournalMetadata, diagnose_committed_journal_metadata, extract_journal_metadata,
        journal_metadata_matches_package, scan_package_journals,
    };
    use crate::core::paths::{ResolvedPaths, resolved_paths};
    use crate::database;
    use crate::models::domains::install::{EngineKind, InstallerType};
    use crate::models::domains::installed::{InstalledPackage, PackageStatus};
    use crate::models::domains::reporting::{
        DiagnosisResult, DiagnosisSeverity, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind,
    };
    use crate::models::domains::shared::DeploymentKind;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::{TempDir, tempdir};

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

        fn build_paths(root: &Path) -> ResolvedPaths {
            let packages = root.join("packages").to_string_lossy().into_owned();
            let data = root.join("data").to_string_lossy().into_owned();
            let logs = root.join("logs").to_string_lossy().into_owned();
            let cache = root.join("cache").to_string_lossy().into_owned();

            resolved_paths(root, &packages, &data, &logs, &cache)
        }

        fn root(&self) -> &Path {
            self._root.path()
        }

        fn pkgdb_root(&self) -> &Path {
            &self.paths.pkgdb
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

        fn journal_path(&self, package_name: &str) -> PathBuf {
            self.pkgdb_root().join(package_name).join("journal.jsonl")
        }
    }

    fn assert_single_diagnosis<'a>(
        diagnostics: &'a [DiagnosisResult],
        expected_error_code: &str,
        expected_severity: DiagnosisSeverity,
    ) -> &'a DiagnosisResult {
        assert_eq!(diagnostics.len(), 1, "expected exactly one diagnosis");

        let diagnosis = &diagnostics[0];
        assert_eq!(diagnosis.error_code, expected_error_code);
        assert_eq!(diagnosis.severity, expected_severity);

        diagnosis
    }

    fn assert_single_recovery_finding(
        findings: &[RecoveryFinding],
        expected_issue_kind: RecoveryIssueKind,
        expected_action_group: Option<RecoveryActionGroup>,
    ) -> &RecoveryFinding {
        assert_eq!(findings.len(), 1, "expected exactly one recovery finding");

        let finding = &findings[0];
        assert_eq!(finding.issue_kind, expected_issue_kind);
        assert_eq!(finding.action_group, expected_action_group);

        finding
    }

    fn assert_recovery_target_path(finding: &RecoveryFinding, expected_path: &Path) {
        let expected_path = expected_path.to_string_lossy().to_string();
        assert_eq!(finding.target_path.as_deref(), Some(expected_path.as_str()));
    }

    fn journal_install_dir(package_name: &str) -> String {
        format!(r"C:\winbrew\apps\{package_name}")
    }

    fn journal_metadata_entry(package_name: &str) -> database::JournalEntry {
        database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: "1.0.0".to_string(),
            engine: "msi".to_string(),
            deployment_kind: DeploymentKind::Installed,
            install_dir: journal_install_dir(package_name),
            dependencies: Vec::new(),
            commands: None,
            bin: None,
            command_resolution: None,
            engine_metadata: None,
        }
    }

    fn journal_commit_entry() -> database::JournalEntry {
        database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        }
    }

    fn write_journal(
        env: &TestEnvironment,
        package_name: &str,
        build: impl FnOnce(&mut database::JournalWriter),
    ) -> PathBuf {
        let mut writer =
            database::JournalWriter::open_for_package(env.root(), package_name, "1.0.0")
                .expect("open journal");
        build(&mut writer);
        writer.flush().expect("flush journal");
        writer.path().to_path_buf()
    }

    fn write_metadata_only_journal(env: &TestEnvironment, package_name: &str) -> PathBuf {
        write_journal(env, package_name, |writer| {
            writer
                .append(&journal_metadata_entry(package_name))
                .expect("write metadata");
        })
    }

    fn write_commit_only_journal(env: &TestEnvironment, package_name: &str) -> PathBuf {
        write_journal(env, package_name, |writer| {
            writer
                .append(&journal_commit_entry())
                .expect("write commit");
        })
    }

    fn write_committed_journal(env: &TestEnvironment, package_name: &str) -> PathBuf {
        write_journal(env, package_name, |writer| {
            writer
                .append(&journal_metadata_entry(package_name))
                .expect("write metadata");
            writer
                .append(&journal_commit_entry())
                .expect("write commit");
        })
    }

    fn write_committed_journal_with_trailing_entry(
        env: &TestEnvironment,
        package_name: &str,
        trailing_path: &str,
    ) -> PathBuf {
        write_journal(env, package_name, |writer| {
            writer
                .append(&journal_metadata_entry(package_name))
                .expect("write metadata");
            writer
                .append(&journal_commit_entry())
                .expect("write commit");
            writer
                .append(&database::JournalEntry::FsCreate {
                    path: trailing_path.to_string(),
                    hash: None,
                })
                .expect("write trailing entry");
        })
    }

    fn sample_package() -> InstalledPackage {
        InstalledPackage {
            name: "Contoso.App".to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Msi,
            deployment_kind: DeploymentKind::Installed,
            engine_kind: EngineKind::Msi,
            engine_metadata: None,
            install_dir: r"C:\winbrew\apps\Contoso.App".to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn extract_journal_metadata_returns_structured_metadata() {
        let entries = vec![
            crate::database::JournalEntry::FsCreate {
                path: r"C:\winbrew\apps\Contoso.App\bin\tool.exe".to_string(),
                hash: None,
            },
            crate::database::JournalEntry::Metadata {
                package_id: "Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
                deployment_kind: DeploymentKind::Installed,
                install_dir: r"C:\winbrew\apps\Contoso.App".to_string(),
                dependencies: Vec::new(),
                commands: None,
                bin: None,
                command_resolution: None,
                engine_metadata: None,
            },
            crate::database::JournalEntry::Commit {
                installed_at: "2026-04-12T00:00:00Z".to_string(),
            },
        ];

        let metadata = extract_journal_metadata(&entries).expect("metadata should be found");

        assert_eq!(metadata.package_id, "Contoso.App");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.engine, "msi");
        assert_eq!(metadata.deployment_kind, DeploymentKind::Installed);
        assert_eq!(metadata.install_dir, r"C:\winbrew\apps\Contoso.App");
    }

    #[test]
    fn extract_journal_metadata_returns_none_when_no_metadata_entry() {
        let entries = vec![journal_commit_entry()];

        assert!(extract_journal_metadata(&entries).is_none());
    }

    #[test]
    fn journal_metadata_matches_package_accepts_matching_package_fields() {
        let package = sample_package();
        let metadata = JournalMetadata {
            package_id: "Contoso.App",
            version: "1.0.0",
            engine: "msi",
            deployment_kind: DeploymentKind::Installed,
            install_dir: r"C:\winbrew\apps\Contoso.App",
        };

        assert!(journal_metadata_matches_package(&package, &metadata));
    }

    #[test]
    fn journal_metadata_matches_package_engine_comparison_is_case_insensitive() {
        let package = sample_package();
        let metadata = JournalMetadata {
            package_id: "Contoso.App",
            version: "1.0.0",
            engine: "MSI",
            deployment_kind: DeploymentKind::Installed,
            install_dir: r"C:\winbrew\apps\Contoso.App",
        };

        assert!(journal_metadata_matches_package(&package, &metadata));
    }

    #[test]
    fn diagnose_committed_journal_metadata_returns_stale_diagnosis_for_changed_package() {
        let package = sample_package();
        let metadata = JournalMetadata {
            package_id: "Contoso.App",
            version: "0.9.0",
            engine: "msi",
            deployment_kind: DeploymentKind::Installed,
            install_dir: r"C:\winbrew\apps\Contoso.App",
        };
        let packages = std::collections::HashMap::from([(package.name.as_str(), &package)]);

        let diagnosis = diagnose_committed_journal_metadata(
            &PathBuf::from(r"C:\winbrew\pkgdb\Contoso.App\journal.jsonl"),
            &metadata,
            &packages,
        )
        .expect("stale package should produce a diagnosis");

        assert_eq!(diagnosis.error_code, "stale_package_journal");
    }

    #[test]
    fn scan_package_journals_detects_incomplete_journal() {
        let env = TestEnvironment::new();

        write_metadata_only_journal(&env, "Contoso.Recover");

        let scan = scan_package_journals(&env.paths, &[]);

        assert_single_diagnosis(
            &scan.diagnostics,
            "incomplete_package_journal",
            DiagnosisSeverity::Error,
        );

        let finding = assert_single_recovery_finding(
            &scan.recovery_findings,
            RecoveryIssueKind::RecoveryTrailMissing,
            None,
        );
        assert!(finding.target_path.is_none());
    }

    #[test]
    fn scan_package_journals_detects_malformed_journal() {
        let env = TestEnvironment::new();

        let journal_path = env.journal_path("Contoso.Malformed");
        env.write_file(&journal_path, b"{not-json}\n");

        let scan = scan_package_journals(&env.paths, &[]);

        assert_single_diagnosis(
            &scan.diagnostics,
            "malformed_package_journal",
            DiagnosisSeverity::Error,
        );

        let finding = assert_single_recovery_finding(
            &scan.recovery_findings,
            RecoveryIssueKind::RecoveryTrailMissing,
            None,
        );
        assert!(finding.target_path.is_none());
    }

    #[test]
    fn scan_package_journals_skips_package_directory_without_journal_file() {
        let env = TestEnvironment::new();

        let journal_dir = env.pkgdb_root().join("Contoso.MissingJournal");
        env.create_dir(&journal_dir);

        let scan = scan_package_journals(&env.paths, &[]);

        assert!(scan.diagnostics.is_empty());
        assert!(scan.recovery_findings.is_empty());
    }

    #[test]
    fn scan_package_journals_reports_missing_journal_metadata() {
        let env = TestEnvironment::new();

        write_commit_only_journal(&env, "Contoso.MissingMeta");

        let scan = scan_package_journals(&env.paths, &[]);

        assert_single_diagnosis(
            &scan.diagnostics,
            "missing_journal_metadata",
            DiagnosisSeverity::Error,
        );
        assert!(scan.recovery_findings.is_empty());
    }

    #[test]
    fn scan_package_journals_detects_orphan_committed_journal() {
        let env = TestEnvironment::new();

        let journal_path = write_committed_journal(&env, "Contoso.Orphan");

        let scan = scan_package_journals(&env.paths, &[]);

        let diagnosis = assert_single_diagnosis(
            &scan.diagnostics,
            "orphan_package_journal",
            DiagnosisSeverity::Warning,
        );
        assert!(diagnosis.description.contains("no installed package"));

        let finding = assert_single_recovery_finding(
            &scan.recovery_findings,
            RecoveryIssueKind::IncompleteInstall,
            Some(RecoveryActionGroup::JournalReplay),
        );
        assert_recovery_target_path(finding, &journal_path);
    }

    #[test]
    fn scan_package_journals_tracks_trailing_journal_replay_target() {
        let env = TestEnvironment::new();

        let journal_path = write_committed_journal_with_trailing_entry(
            &env,
            "Contoso.Trailing",
            r"C:\winbrew\apps\Contoso.Trailing\payload.exe",
        );

        let scan = scan_package_journals(&env.paths, &[]);

        let diagnosis = assert_single_diagnosis(
            &scan.diagnostics,
            "trailing_package_journal",
            DiagnosisSeverity::Error,
        );
        assert!(
            diagnosis
                .description
                .contains("trailing entries after commit")
        );

        let finding = assert_single_recovery_finding(
            &scan.recovery_findings,
            RecoveryIssueKind::Conflict,
            Some(RecoveryActionGroup::JournalReplay),
        );
        assert_recovery_target_path(finding, &journal_path);
    }
}
