use anyhow::Result;
use std::path::Path;
use tempfile::tempdir;
use winbrew_database as database;
use winbrew_models::domains::install::{EngineInstallReceipt, EngineKind, InstallerType};
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
        install_dir: format!(r"C:\winbrew\packages\{name}"),
        dependencies: Vec::new(),
        status,
        installed_at: "2026-04-19T00:00:00Z".to_string(),
    }
}

#[test]
fn command_registry_round_trip_and_reverse_lookup() -> Result<()> {
    let test_root = tempdir()?;
    init_database(test_root.path())?;

    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;

    let package = sample_package("Contoso.Commands", PackageStatus::Installing);
    database::insert_package(&conn, &package)?;

    let receipt =
        EngineInstallReceipt::new(EngineKind::Portable, package.install_dir.clone(), None);

    let mut conn = conn;
    database::commit_install_with_commands(
        &mut conn,
        &package.name,
        &receipt,
        Some(r#"["grep", "git"]"#),
    )?;

    assert_eq!(
        database::find_command_owner(&conn, "grep")?,
        Some(package.name.clone())
    );
    assert_eq!(
        database::find_command_owner(&conn, "git")?,
        Some(package.name.clone())
    );
    assert_eq!(
        database::list_commands_for_package(&conn, &package.name)?,
        vec!["git".to_string(), "grep".to_string()]
    );
    assert_eq!(
        database::get_package_command_names(&conn, &package.name)?,
        Some(vec!["git".to_string(), "grep".to_string()])
    );

    assert!(database::delete_package(&conn, &package.name)?);
    assert!(database::find_command_owner(&conn, "grep")?.is_none());
    assert!(database::list_commands_for_package(&conn, &package.name)?.is_empty());
    assert!(database::get_package_command_names(&conn, &package.name)?.is_none());

    Ok(())
}

#[test]
fn command_registry_commit_conflict_surfaces_as_claimed_during_install() -> Result<()> {
    let test_root = tempdir()?;
    init_database(test_root.path())?;

    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;

    let owner = sample_package("Contoso.Owner", PackageStatus::Installing);
    database::insert_package(&conn, &owner)?;
    let owner_receipt =
        EngineInstallReceipt::new(EngineKind::Portable, owner.install_dir.clone(), None);

    let mut conn = conn;
    database::commit_install_with_commands(
        &mut conn,
        &owner.name,
        &owner_receipt,
        Some(r#"["grep"]"#),
    )?;

    let contender = sample_package("Contoso.Contender", PackageStatus::Installing);
    database::insert_package(&conn, &contender)?;
    let contender_receipt =
        EngineInstallReceipt::new(EngineKind::Portable, contender.install_dir.clone(), None);

    let err = database::commit_install_with_commands(
        &mut conn,
        &contender.name,
        &contender_receipt,
        Some(r#"["grep"]"#),
    )
    .expect_err("conflicting command should fail");

    assert!(
        err.downcast_ref::<database::CommandRegistryConflictError>()
            .is_some()
    );

    let stored = database::get_package(&conn, &contender.name)?
        .expect("contender package should still exist after rollback");
    assert_eq!(stored.status, PackageStatus::Installing);
    assert!(database::list_commands_for_package(&conn, &contender.name)?.is_empty());
    assert!(database::get_package_command_names(&conn, &contender.name)?.is_none());

    Ok(())
}
