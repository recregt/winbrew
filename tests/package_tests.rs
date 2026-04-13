#[path = "common/mod.rs"]
mod common;

use anyhow::Result;
use common::db::{init_database, reset_installed_packages};
use common::shared_root::test_root;
use winbrew::database;
use winbrew_models::domains::install::{
    EngineKind, EngineMetadata, InstallScope, InstallerType,
};
use winbrew_models::domains::installed::{InstalledPackage as Package, PackageStatus};

fn sample_package(name: &str, status: PackageStatus) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: InstallerType::Portable,
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
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;

    let conn = database::get_conn()?;
    reset_installed_packages(&conn)?;
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
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;

    let conn = database::get_conn()?;
    reset_installed_packages(&conn)?;
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
