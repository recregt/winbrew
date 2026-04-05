#[path = "common/shared_root.rs"]
mod shared_root;

use anyhow::Result;
use shared_root::test_root;
use std::fs;
use std::path::Path;
use winbrew::database;
use winbrew::models::{Package, PackageStatus};
use winbrew::services::remove;

fn sample_package(
    name: &str,
    kind: &str,
    install_dir: &Path,
    dependencies: Vec<String>,
) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: kind.to_string(),
        install_dir: install_dir.to_string_lossy().into_owned(),
        msix_package_full_name: None,
        dependencies,
        status: PackageStatus::Ok,
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

fn init_database(root: &Path) -> Result<()> {
    let config = database::Config::load_at(root)?;
    database::init(&config.resolved_paths())
}

#[test]
fn remove_deletes_portable_installation_and_database_row() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_database(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Remove");
    fs::create_dir_all(&install_dir)?;
    fs::write(install_dir.join("tool.exe"), b"binary")?;

    let package = sample_package("Contoso.Remove", "portable", &install_dir, Vec::new());

    database::insert_package(&conn, &package)?;

    remove::remove(&package.name, false)?;

    assert!(!install_dir.exists());
    assert!(database::get_package(&conn, &package.name)?.is_none());
    assert!(database::list_packages(&conn)?.is_empty());

    Ok(())
}

#[test]
fn remove_blocks_packages_with_dependents_without_force() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_database(root)?;
    let conn = database::get_conn()?;

    let target_install_dir = root.join("packages").join("Contoso.Target");
    let dependent_install_dir = root.join("packages").join("Contoso.Dependent");

    let target = sample_package(
        "Contoso.Target",
        "portable",
        &target_install_dir,
        Vec::new(),
    );
    let dependent = sample_package(
        "Contoso.Dependent",
        "portable",
        &dependent_install_dir,
        vec!["Contoso.Target@1.0.0".to_string()],
    );

    database::insert_package(&conn, &target)?;
    database::insert_package(&conn, &dependent)?;

    let err =
        remove::remove(&target.name, false).expect_err("dependent package should block removal");

    assert!(err.to_string().contains("required by"));
    assert!(database::get_package(&conn, &target.name)?.is_some());
    assert!(database::get_package(&conn, &dependent.name)?.is_some());

    Ok(())
}

#[test]
fn find_dependents_returns_sorted_packages() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_database(root)?;
    let conn = database::get_conn()?;

    let target_install_dir = root.join("packages").join("Contoso.Base");
    let alpha_install_dir = root.join("packages").join("Alpha.Consumer");
    let beta_install_dir = root.join("packages").join("Beta.Consumer");
    let gamma_install_dir = root.join("packages").join("Gamma.Consumer");

    database::insert_package(
        &conn,
        &sample_package("Contoso.Base", "portable", &target_install_dir, Vec::new()),
    )?;
    database::insert_package(
        &conn,
        &sample_package(
            "Gamma.Consumer",
            "portable",
            &gamma_install_dir,
            vec!["Contoso.Base@1.0.0".to_string()],
        ),
    )?;
    database::insert_package(
        &conn,
        &sample_package(
            "Alpha.Consumer",
            "portable",
            &alpha_install_dir,
            vec!["Contoso.Base@1.0.0".to_string()],
        ),
    )?;
    database::insert_package(
        &conn,
        &sample_package(
            "Beta.Consumer",
            "portable",
            &beta_install_dir,
            vec!["Contoso.Base@1.0.0".to_string()],
        ),
    )?;

    let dependents = remove::find_dependents("Contoso.Base", &conn)?;

    assert_eq!(
        dependents,
        vec![
            "Alpha.Consumer".to_string(),
            "Beta.Consumer".to_string(),
            "Gamma.Consumer".to_string(),
        ]
    );

    Ok(())
}

#[test]
fn remove_rejects_unsupported_package_type() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_database(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Unsupported");
    let package = sample_package("Contoso.Unsupported", "exe", &install_dir, Vec::new());

    database::insert_package(&conn, &package)?;

    let err = remove::remove(&package.name, false).expect_err("unsupported kind should fail");

    assert!(err.to_string().contains("unsupported package type"));
    assert!(database::get_package(&conn, &package.name)?.is_some());

    Ok(())
}
