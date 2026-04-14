use crate::core::paths::resolved_paths;
use crate::database;
use crate::models::domains::install::InstallerType;
use crate::models::domains::installed::{InstalledPackage, PackageStatus};
use crate::models::domains::reporting::{
    DiagnosisSeverity, RecoveryActionGroup, RecoveryIssueKind,
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;
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

fn init_storage(root: &Path) {
    let packages = root.join("packages").to_string_lossy().into_owned();
    let data = root.join("data").to_string_lossy().into_owned();
    let logs = root.join("logs").to_string_lossy().into_owned();
    let cache = root.join("cache").to_string_lossy().into_owned();
    let paths = resolved_paths(root, &packages, &data, &logs, &cache);

    database::init(&paths).expect("database should initialize");
}

fn resolved_root_paths(root: &Path) -> crate::core::paths::ResolvedPaths {
    let packages = root.join("packages").to_string_lossy().into_owned();
    let data = root.join("data").to_string_lossy().into_owned();
    let logs = root.join("logs").to_string_lossy().into_owned();
    let cache = root.join("cache").to_string_lossy().into_owned();

    resolved_paths(root, &packages, &data, &logs, &cache)
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
    let temp_dir = tempdir().expect("temp dir should be created");
    let packages_root = temp_dir.path().join("packages");
    std::fs::create_dir_all(&packages_root).expect("packages root should be created");

    let orphan_dir = packages_root.join("Contoso.Orphan");
    std::fs::create_dir_all(&orphan_dir).expect("orphan dir should be created");

    let packages = vec![sample_package(
        "Contoso.Known",
        &packages_root.join("Contoso.Known"),
    )];

    let scan = super::scan_orphaned_install_dirs(&packages_root, &packages);

    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].error_code, "orphan_install_directory");
    assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Warning);
    assert!(scan.diagnostics[0].description.contains("Contoso.Orphan"));
    assert_eq!(scan.recovery_findings.len(), 1);
    assert_eq!(
        scan.recovery_findings[0].action_group,
        Some(RecoveryActionGroup::OrphanCleanup)
    );
    let orphan_path = orphan_dir.to_string_lossy().to_string();
    assert_eq!(
        scan.recovery_findings[0].target_path.as_deref(),
        Some(orphan_path.as_str())
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

    let mut conn = database::get_conn().expect("database connection");
    database::insert_package(&conn, &package).expect("insert package");
    database::replace_snapshot(&mut conn, &snapshot).expect("replace snapshot");

    let scan = super::scan_msi_inventory(&conn, &[package]);

    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].error_code, "msi_file_hash_mismatch");
    assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Error);
    assert!(scan.diagnostics[0].description.contains("Contoso.Msi"));
    assert_eq!(scan.recovery_findings.len(), 1);
    assert_eq!(
        scan.recovery_findings[0].action_group,
        Some(RecoveryActionGroup::FileRestore)
    );
    let expected_file_path = file_path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    assert_eq!(
        scan.recovery_findings[0].target_path.as_deref(),
        Some(expected_file_path.as_str())
    );
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

    let mut conn = database::get_conn().expect("database connection");
    database::insert_package(&conn, &package).expect("insert package");
    database::replace_snapshot(&mut conn, &snapshot).expect("replace snapshot");

    let scan = super::scan_msi_inventory(&conn, &[package]);

    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].error_code, "missing_msi_file");
    assert_eq!(scan.diagnostics[0].severity, DiagnosisSeverity::Error);
    assert!(scan.diagnostics[0].description.contains("Contoso.Msi"));
    assert_eq!(scan.recovery_findings.len(), 1);
    assert_eq!(
        scan.recovery_findings[0].action_group,
        Some(RecoveryActionGroup::FileRestore)
    );
    let expected_file_path = install_dir
        .join("bin")
        .join("demo.exe")
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    assert_eq!(
        scan.recovery_findings[0].target_path.as_deref(),
        Some(expected_file_path.as_str())
    );
}

#[test]
fn scan_package_journals_detects_incomplete_journal() {
    let root = tempdir().expect("temp root");
    let paths = resolved_root_paths(root.path());

    let mut writer =
        database::JournalWriter::open_for_package(root.path(), "Contoso.Recover", "1.0.0")
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

    let scan = super::scan_package_journals(&paths, &[]);

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
    let paths = resolved_root_paths(root.path());

    let mut writer =
        database::JournalWriter::open_for_package(root.path(), "Contoso.Orphan", "1.0.0")
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

    let scan = super::scan_package_journals(&paths, &[]);

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
    let journal_path_string = journal_path.to_string_lossy().to_string();
    assert_eq!(
        scan.recovery_findings[0].target_path.as_deref(),
        Some(journal_path_string.as_str())
    );
}

#[test]
fn scan_package_journals_tracks_trailing_journal_replay_target() {
    let root = tempdir().expect("temp root");
    let paths = resolved_root_paths(root.path());

    let mut writer =
        database::JournalWriter::open_for_package(root.path(), "Contoso.Trailing", "1.0.0")
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

    let scan = super::scan_package_journals(&paths, &[]);

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
    let journal_path_string = journal_path.to_string_lossy().to_string();
    assert_eq!(
        scan.recovery_findings[0].target_path.as_deref(),
        Some(journal_path_string.as_str())
    );
}
