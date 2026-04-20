use anyhow::Result;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use winbrew_database as database;
use winbrew_models::domains::install::{EngineKind, EngineMetadata, InstallScope, InstallerType};
use winbrew_models::domains::installed::{InstalledPackage as Package, PackageStatus};

fn init_database(root: &Path) -> Result<()> {
    let config = database::Config::load_at(root)?;
    database::init(&config.resolved_paths())?;
    Ok(())
}

fn sample_package(name: &str, status: PackageStatus) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: InstallerType::Portable,
        deployment_kind: InstallerType::Portable.deployment_kind(),
        engine_kind: EngineKind::Portable,
        engine_metadata: None,
        install_dir: format!(r"C:\\winbrew\\packages\\{name}"),
        dependencies: vec!["dep-a".to_string(), "dep-b".to_string()],
        status,
        installed_at: "2026-03-24T00:00:00Z".to_string(),
    }
}

#[test]
fn package_crud_round_trip() -> Result<()> {
    let test_root = tempdir()?;
    init_database(test_root.path())?;

    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;
    let package = sample_package("Contoso.RoundTrip", PackageStatus::Installing);

    database::insert_package(&conn, &package)?;

    let stored = database::get_package(&conn, &package.name)?.expect("package should exist");
    assert_eq!(stored.name, package.name);
    assert_eq!(stored.version, package.version);
    assert_eq!(stored.kind, package.kind);
    assert_eq!(stored.engine_kind, package.engine_kind);
    assert_eq!(stored.install_dir, package.install_dir);
    assert_eq!(stored.engine_metadata, package.engine_metadata);
    assert_eq!(stored.dependencies, package.dependencies);
    assert_eq!(stored.status, PackageStatus::Installing);

    assert!(database::list_packages(&conn)?.is_empty());

    database::update_status(&conn, &package.name, PackageStatus::Ok)?;

    let listed = database::list_packages(&conn)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, package.name);
    assert_eq!(listed[0].status, PackageStatus::Ok);

    assert!(database::delete_package(&conn, &package.name)?);
    assert!(database::get_package(&conn, &package.name)?.is_none());
    assert!(!database::delete_package(&conn, &package.name)?);

    Ok(())
}

#[test]
fn update_status_and_engine_metadata_round_trip() -> Result<()> {
    let test_root = tempdir()?;
    init_database(test_root.path())?;

    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;
    let mut package = sample_package("Contoso.Msix", PackageStatus::Installing);
    package.engine_kind = EngineKind::Msix;
    package.kind = InstallerType::Msix;

    database::insert_package(&conn, &package)?;

    database::update_status_and_engine_metadata(
        &conn,
        &package.name,
        PackageStatus::Ok,
        Some(&EngineMetadata::msix(
            "Contoso.Msix_1.0.0_x64__8wekyb3d8bbwe",
            InstallScope::Installed,
        )),
        &package.install_dir,
        "2026-03-24T00:10:00Z",
    )?;

    let stored = database::get_package(&conn, &package.name)?.expect("package should exist");
    assert_eq!(stored.status, PackageStatus::Ok);
    assert_eq!(stored.engine_kind, EngineKind::Msix);
    assert_eq!(
        stored.engine_metadata,
        Some(EngineMetadata::msix(
            "Contoso.Msix_1.0.0_x64__8wekyb3d8bbwe",
            InstallScope::Installed,
        ))
    );

    Ok(())
}

#[test]
fn update_status_and_engine_metadata_round_trip_native_exe() -> Result<()> {
    let test_root = tempdir()?;
    init_database(test_root.path())?;

    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;
    let mut package = sample_package("Contoso.NativeExe", PackageStatus::Installing);
    package.engine_kind = EngineKind::NativeExe;
    package.kind = InstallerType::Exe;

    database::insert_package(&conn, &package)?;

    database::update_status_and_engine_metadata(
        &conn,
        &package.name,
        PackageStatus::Ok,
        Some(&EngineMetadata::native_exe(
            Some("C:\\Apps\\Contoso.NativeExe\\uninstall.exe /S".to_string()),
            Some("C:\\Apps\\Contoso.NativeExe\\uninstall.exe".to_string()),
        )),
        &package.install_dir,
        "2026-03-24T00:10:00Z",
    )?;

    let stored = database::get_package(&conn, &package.name)?.expect("package should exist");
    assert_eq!(stored.status, PackageStatus::Ok);
    assert_eq!(stored.engine_kind, EngineKind::NativeExe);
    assert_eq!(
        stored.engine_metadata,
        Some(EngineMetadata::native_exe(
            Some("C:\\Apps\\Contoso.NativeExe\\uninstall.exe /S".to_string()),
            Some("C:\\Apps\\Contoso.NativeExe\\uninstall.exe".to_string()),
        ))
    );

    Ok(())
}

#[test]
fn replay_committed_journal_replaces_existing_package() -> Result<()> {
    let test_root = tempdir()?;
    init_database(test_root.path())?;

    let mut conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;

    let package_name = "Contoso.Replay";
    let install_dir = format!(r"C:\\winbrew\\packages\\{package_name}");
    let package = Package {
        name: package_name.to_string(),
        version: "0.9.0".to_string(),
        kind: InstallerType::Portable,
        deployment_kind: InstallerType::Portable.deployment_kind(),
        engine_kind: EngineKind::Portable,
        engine_metadata: None,
        install_dir: install_dir.clone(),
        dependencies: Vec::new(),
        status: PackageStatus::Installing,
        installed_at: "2026-03-24T00:00:00Z".to_string(),
    };

    database::insert_package(&conn, &package)?;

    let replay_package = Package {
        version: "1.0.0".to_string(),
        status: PackageStatus::Ok,
        ..package.clone()
    };

    let replay = database::CommittedJournalPackage {
        journal_path: PathBuf::from("C:/tmp/journal.jsonl"),
        entries: Vec::new(),
        package: replay_package,
        commands: None,
        bin: Some(vec!["bin/tool.exe".to_string()]),
    };

    database::replay_committed_journal(&mut conn, &replay)?;

    let stored_package =
        database::get_package(&conn, package_name)?.expect("package should exist after replay");
    assert_eq!(stored_package.version, "1.0.0");
    assert_eq!(stored_package.status, PackageStatus::Ok);

    assert_eq!(stored_package.install_dir, install_dir);

    Ok(())
}
