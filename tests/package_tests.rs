#[path = "common/mod.rs"]
mod common;
#[path = "common/env.rs"]
mod test_env;

use anyhow::Result;
use common::env_lock;
use tempfile::tempdir;
use test_env::TestEnvVar;
use winbrew::database;
use winbrew::models::{Package, PackageStatus};

fn sample_package(name: &str, status: PackageStatus) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: "portable".to_string(),
        install_dir: format!(r"C:\\winbrew\\packages\\{name}"),
        msix_package_full_name: Some(format!("{name}_1.0.0_x64__8wekyb3d8bbwe")),
        dependencies: vec!["dep-a".to_string(), "dep-b".to_string()],
        status,
        installed_at: "2026-03-24T00:00:00Z".to_string(),
    }
}

#[test]
fn package_crud_round_trip() -> Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set(
        "WINBREW_PATHS_ROOT",
        temp_root.path().to_string_lossy().as_ref(),
    );

    let conn = database::get_conn()?;
    let package = sample_package("Contoso.RoundTrip", PackageStatus::Installing);

    database::insert_package(&conn, &package)?;

    let stored = database::get_package(&conn, &package.name)?.expect("package should exist");
    assert_eq!(stored.name, package.name);
    assert_eq!(stored.version, package.version);
    assert_eq!(stored.kind, package.kind);
    assert_eq!(stored.install_dir, package.install_dir);
    assert_eq!(
        stored.msix_package_full_name,
        package.msix_package_full_name
    );
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
fn update_status_and_msix_package_full_name_round_trip() -> Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set(
        "WINBREW_PATHS_ROOT",
        temp_root.path().to_string_lossy().as_ref(),
    );

    let conn = database::get_conn()?;
    let mut package = sample_package("Contoso.Msix", PackageStatus::Installing);
    package.msix_package_full_name = None;

    database::insert_package(&conn, &package)?;

    database::update_status_and_msix_package_full_name(
        &conn,
        &package.name,
        PackageStatus::Ok,
        Some("Contoso.Msix_1.0.0_x64__8wekyb3d8bbwe"),
    )?;

    let stored = database::get_package(&conn, &package.name)?.expect("package should exist");
    assert_eq!(stored.status, PackageStatus::Ok);
    assert_eq!(
        stored.msix_package_full_name,
        Some("Contoso.Msix_1.0.0_x64__8wekyb3d8bbwe".to_string())
    );

    Ok(())
}
