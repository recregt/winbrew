use crate::core::paths::{ResolvedPaths, resolved_paths};
use crate::database;
use crate::models::domains::install::InstallerType;
use crate::models::domains::installed::{InstalledPackage, PackageStatus};
use crate::models::domains::reporting::{
    DiagnosisSeverity, RecoveryActionGroup, RecoveryIssueKind,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};
use winbrew_models::domains::install::EngineKind;
use winbrew_models::domains::inventory::{
    MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
    MsiRegistryRecord, MsiShortcutRecord,
};

fn sample_package(name: &str, install_dir: &std::path::Path) -> InstalledPackage {
    InstalledPackage {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: InstallerType::Portable,
        deployment_kind: InstallerType::Portable.deployment_kind(),
        engine_kind: InstallerType::Portable.into(),
        engine_metadata: None,
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies: Vec::new(),
        status: PackageStatus::Ok,
        installed_at: "2026-04-05T00:00:00Z".to_string(),
    }
}

fn sample_msi_package(name: &str, install_dir: &std::path::Path) -> InstalledPackage {
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
            hash_algorithm: Some(winbrew_models::domains::shared::HashAlgorithm::Sha256),
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

    fn root(&self) -> &Path {
        self._root.path()
    }

    fn packages_root(&self) -> &Path {
        &self.paths.packages
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

    fn db_conn(&self) -> database::DbConnection {
        database::get_conn().expect("database connection")
    }

    fn insert_package(&self, package: &InstalledPackage) -> database::DbConnection {
        let conn = self.db_conn();
        database::insert_package(&conn, package).expect("insert package");
        conn
    }

    fn make_portable_package(&self, name: &str) -> (InstalledPackage, PathBuf) {
        let install_dir = self.packages_root().join(name);
        (sample_package(name, &install_dir), install_dir)
    }

    fn make_msi_package(&self, name: &str) -> (InstalledPackage, PathBuf) {
        let install_dir = self.packages_root().join(name);
        (sample_msi_package(name, &install_dir), install_dir)
    }

    fn make_msi_snapshot(
        &self,
        name: &str,
        install_dir: &Path,
        hash_hex: &str,
    ) -> MsiInventorySnapshot {
        sample_snapshot(name, install_dir, hash_hex)
    }

    fn journal_path(&self, package_name: &str) -> PathBuf {
        self.pkgdb_root().join(package_name).join("journal.jsonl")
    }
}

fn assert_single_diagnosis<'a>(
    diagnostics: &'a [crate::models::domains::reporting::DiagnosisResult],
    expected_error_code: &str,
    expected_severity: DiagnosisSeverity,
) -> &'a crate::models::domains::reporting::DiagnosisResult {
    assert_eq!(diagnostics.len(), 1, "expected exactly one diagnosis");

    let diagnosis = &diagnostics[0];
    assert_eq!(diagnosis.error_code, expected_error_code);
    assert_eq!(diagnosis.severity, expected_severity);

    diagnosis
}

fn assert_single_recovery_finding(
    findings: &[crate::models::domains::reporting::RecoveryFinding],
    expected_issue_kind: RecoveryIssueKind,
    expected_action_group: Option<RecoveryActionGroup>,
) -> &crate::models::domains::reporting::RecoveryFinding {
    assert_eq!(findings.len(), 1, "expected exactly one recovery finding");

    let finding = &findings[0];
    assert_eq!(finding.issue_kind, expected_issue_kind);
    assert_eq!(finding.action_group, expected_action_group);

    finding
}

fn assert_recovery_target_path(
    finding: &crate::models::domains::reporting::RecoveryFinding,
    expected_path: &Path,
) {
    let expected_path = expected_path.to_string_lossy().to_string();
    assert_eq!(finding.target_path.as_deref(), Some(expected_path.as_str()));
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

#[test]
fn scan_packages_sorts_diagnoses_by_error_code() {
    let env = TestEnvironment::new();
    let (zeta_missing, _zeta_dir) = env.make_portable_package("Zeta.Missing");
    let (alpha_valid, valid_dir) = env.make_portable_package("Alpha.Valid");
    let (beta_missing, _beta_dir) = env.make_portable_package("Beta.Missing");
    env.create_dir(&valid_dir);

    let packages = vec![zeta_missing, alpha_valid, beta_missing];

    let scan = super::scan_packages(&packages);

    assert_eq!(scan.diagnostics.len(), 2);
    assert_eq!(scan.diagnostics[0].error_code, "missing_install_directory");
    assert_eq!(scan.diagnostics[1].error_code, "missing_install_directory");
    assert_eq!(scan.recovery_findings.len(), 2);
    assert_eq!(
        scan.recovery_findings[0].action_group,
        Some(RecoveryActionGroup::Reinstall)
    );
    assert_eq!(
        scan.recovery_findings[1].action_group,
        Some(RecoveryActionGroup::Reinstall)
    );
}

#[test]
fn scan_orphaned_install_dirs_detects_directories_without_packages() {
    let env = TestEnvironment::new();
    env.create_dir(env.packages_root());

    let orphan_dir = env.packages_root().join("Contoso.Orphan");
    env.create_dir(&orphan_dir);

    let (known_package, _known_dir) = env.make_portable_package("Contoso.Known");
    let packages = vec![known_package];

    let scan = super::scan_orphaned_install_dirs(env.packages_root(), &packages);

    let diagnosis = assert_single_diagnosis(
        &scan.diagnostics,
        "orphan_install_directory",
        DiagnosisSeverity::Warning,
    );
    assert!(diagnosis.description.contains("Contoso.Orphan"));

    let finding = assert_single_recovery_finding(
        &scan.recovery_findings,
        RecoveryIssueKind::IncompleteInstall,
        Some(RecoveryActionGroup::OrphanCleanup),
    );
    assert_recovery_target_path(finding, &orphan_dir);
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

    let scan = super::scan_msi_inventory(&conn, &[package]);

    let diagnosis = assert_single_diagnosis(
        &scan.diagnostics,
        "msi_file_hash_mismatch",
        DiagnosisSeverity::Error,
    );
    assert!(diagnosis.description.contains("Contoso.Msi"));

    let finding = assert_single_recovery_finding(
        &scan.recovery_findings,
        RecoveryIssueKind::DiskDrift,
        Some(RecoveryActionGroup::FileRestore),
    );
    assert_normalized_recovery_target_path(finding, &file_path);
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

    let scan = super::scan_msi_inventory(&conn, &[package]);

    let diagnosis = assert_single_diagnosis(
        &scan.diagnostics,
        "missing_msi_file",
        DiagnosisSeverity::Error,
    );
    assert!(diagnosis.description.contains("Contoso.Msi"));

    let finding = assert_single_recovery_finding(
        &scan.recovery_findings,
        RecoveryIssueKind::DiskDrift,
        Some(RecoveryActionGroup::FileRestore),
    );
    assert_normalized_recovery_target_path(finding, &install_dir.join("bin").join("demo.exe"));
}

#[test]
fn scan_package_journals_detects_incomplete_journal() {
    let env = TestEnvironment::new();

    let mut writer =
        database::JournalWriter::open_for_package(env.root(), "Contoso.Recover", "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: "Contoso.Recover".to_string(),
            version: "1.0.0".to_string(),
            engine: "msi".to_string(),
            deployment_kind: winbrew_models::shared::DeploymentKind::Installed,
            install_dir: r"C:\winbrew\apps\Contoso.Recover".to_string(),
            dependencies: Vec::new(),
            engine_metadata: None,
        })
        .expect("write metadata");
    writer.flush().expect("flush journal");

    let scan = super::scan_package_journals(&env.paths, &[]);

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

    let scan = super::scan_package_journals(&env.paths, &[]);

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
fn scan_package_journals_reports_missing_journal_metadata() {
    let env = TestEnvironment::new();

    let mut writer =
        database::JournalWriter::open_for_package(env.root(), "Contoso.MissingMeta", "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    let scan = super::scan_package_journals(&env.paths, &[]);

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

    let mut writer =
        database::JournalWriter::open_for_package(env.root(), "Contoso.Orphan", "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: "Contoso.Orphan".to_string(),
            version: "1.0.0".to_string(),
            engine: "msi".to_string(),
            deployment_kind: winbrew_models::shared::DeploymentKind::Installed,
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

    let scan = super::scan_package_journals(&env.paths, &[]);

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

    let mut writer =
        database::JournalWriter::open_for_package(env.root(), "Contoso.Trailing", "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: "Contoso.Trailing".to_string(),
            version: "1.0.0".to_string(),
            engine: "msi".to_string(),
            deployment_kind: winbrew_models::shared::DeploymentKind::Installed,
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

    let scan = super::scan_package_journals(&env.paths, &[]);

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
