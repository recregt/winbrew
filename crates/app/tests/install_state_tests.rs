use anyhow::Result;
use std::fs;
use std::path::Path;

use winbrew_app::database;
use winbrew_app::install::state;
use winbrew_models::domains::install::{EngineInstallReceipt, EngineKind, InstallerType};
use winbrew_models::domains::installed::{InstalledPackage as Package, PackageStatus};
use winbrew_testing::{init_database, reset_install_state, test_root};

fn sample_package(name: &str, status: PackageStatus, install_dir: &Path) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: InstallerType::Portable,
        deployment_kind: InstallerType::Portable.deployment_kind(),
        engine_kind: EngineKind::Portable,
        engine_metadata: None,
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies: Vec::new(),
        status,
        installed_at: "2026-04-05T00:00:00Z".to_string(),
    }
}

#[test]
fn prepare_install_target_removes_orphaned_directory() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Orphan");
    fs::create_dir_all(&install_dir)?;
    fs::write(install_dir.join("tool.exe"), b"binary")?;

    state::prepare_install_target(&conn, "Contoso.Orphan", &install_dir)?;

    assert!(!install_dir.exists());
    assert!(database::get_package(&conn, "Contoso.Orphan")?.is_none());

    Ok(())
}

#[test]
fn prepare_install_target_deletes_failed_package_and_directory() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Failed");
    fs::create_dir_all(&install_dir)?;
    fs::write(install_dir.join("tool.exe"), b"binary")?;

    let package = sample_package("Contoso.Failed", PackageStatus::Failed, &install_dir);
    database::insert_package(&conn, &package)?;

    state::prepare_install_target(&conn, &package.name, &install_dir)?;

    assert!(!install_dir.exists());
    assert!(database::get_package(&conn, &package.name)?.is_none());

    Ok(())
}

#[test]
fn prepare_install_target_rejects_installed_package() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Exists");
    let package = sample_package("Contoso.Exists", PackageStatus::Ok, &install_dir);
    database::insert_package(&conn, &package)?;

    let err = state::prepare_install_target(&conn, &package.name, &install_dir)
        .expect_err("installed package should be rejected");

    assert!(err.to_string().contains("already installed"));
    assert!(database::get_package(&conn, &package.name)?.is_some());

    Ok(())
}

#[test]
fn mark_installing_and_mark_ok_update_status() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Installing");

    state::mark_installing(
        &conn,
        "Contoso.Installing",
        "2.4.6",
        InstallerType::Portable,
        InstallerType::Portable.deployment_kind(),
        EngineKind::Portable,
        &install_dir,
    )?;

    let stored = database::get_package(&conn, "Contoso.Installing")?
        .expect("package should exist after mark_installing");
    assert_eq!(stored.status, PackageStatus::Installing);
    assert_eq!(stored.engine_kind, EngineKind::Portable);
    assert_eq!(stored.dependencies, Vec::<String>::new());

    let receipt = EngineInstallReceipt::new(
        EngineKind::Portable,
        install_dir.to_string_lossy().into_owned(),
        None,
    );

    let mut conn = conn;
    database::commit_install(&mut conn, "Contoso.Installing", &receipt)?;

    let stored = database::get_package(&conn, "Contoso.Installing")?
        .expect("package should still exist after mark_ok");
    assert_eq!(stored.status, PackageStatus::Ok);

    Ok(())
}
