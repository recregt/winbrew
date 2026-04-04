#[path = "common/mod.rs"]
mod common;
#[path = "common/shared_root.rs"]
mod shared_root;

use anyhow::Result;
use common::env_lock;
use shared_root::shared_test_root;
use std::fs;
use std::path::Path;
use winbrew::database;
use winbrew::models::{Package, PackageStatus};
use winbrew::services::install::state;

fn sample_package(name: &str, status: PackageStatus, install_dir: &Path) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: "portable".to_string(),
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies: Vec::new(),
        status,
        installed_at: "2026-04-05T00:00:00Z".to_string(),
    }
}

fn reset_database(root: &Path) -> Result<()> {
    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;

    let packages_dir = root.join("packages");
    if packages_dir.exists() {
        fs::remove_dir_all(&packages_dir)?;
    }
    fs::create_dir_all(&packages_dir)?;

    Ok(())
}

#[test]
fn prepare_install_target_removes_orphaned_directory() -> Result<()> {
    let _guard = env_lock();
    let root = shared_test_root();
    reset_database(root)?;
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
    let _guard = env_lock();
    let root = shared_test_root();
    reset_database(root)?;
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
    let _guard = env_lock();
    let root = shared_test_root();
    reset_database(root)?;
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
    let _guard = env_lock();
    let root = shared_test_root();
    reset_database(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Installing");

    state::mark_installing(
        &conn,
        "Contoso.Installing",
        "2.4.6",
        "portable",
        &install_dir,
    )?;

    let stored = database::get_package(&conn, "Contoso.Installing")?
        .expect("package should exist after mark_installing");
    assert_eq!(stored.status, PackageStatus::Installing);
    assert_eq!(stored.dependencies, Vec::<String>::new());

    state::mark_ok(&conn, "Contoso.Installing")?;

    let stored = database::get_package(&conn, "Contoso.Installing")?
        .expect("package should still exist after mark_ok");
    assert_eq!(stored.status, PackageStatus::Ok);

    Ok(())
}
