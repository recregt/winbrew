use anyhow::Result;
use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};

use winbrew_app::install;
use winbrew_app::install::InstallObserver;
use winbrew_app::remove;
use winbrew_app::{AppContext, database};
use winbrew_models::domains::catalog::CatalogPackage;
use winbrew_models::domains::install::{EngineMetadata, InstallerType};
use winbrew_models::domains::installed::{InstalledPackage as Package, PackageStatus};
use winbrew_models::domains::package::{PackageName, PackageRef};
use winbrew_testing::{
    MockServer, catalog_package_id, init_database, reset_install_state,
    seed_catalog_db_with_installer, sha512_hex, system_font_file_name, system_font_path, test_root,
};

struct NoopInstallObserver;

impl InstallObserver for NoopInstallObserver {
    fn choose_package(
        &mut self,
        _query: &str,
        _matches: &[CatalogPackage],
    ) -> anyhow::Result<usize> {
        unreachable!("install should not prompt for an exact match")
    }

    fn on_start(&mut self, _total_bytes: Option<u64>) {}

    fn on_progress(&mut self, _downloaded_bytes: u64) {}
}

fn font_fixture_prefix(root: &Path, base: &str) -> String {
    let root_suffix = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("font");
    let sanitized_suffix: String = root_suffix
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();

    format!("{}-{}", base, sanitized_suffix)
}

fn sample_package(
    name: &str,
    kind: InstallerType,
    install_dir: &Path,
    dependencies: Vec<String>,
) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind,
        deployment_kind: kind.deployment_kind(),
        engine_kind: kind.into(),
        engine_metadata: None,
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies,
        status: PackageStatus::Ok,
        installed_at: "2026-04-05T00:00:00Z".to_string(),
    }
}

fn seed_catalog_commands(
    catalog_db_path: &Path,
    package_name: &str,
    commands_json: &str,
) -> Result<()> {
    let conn = Connection::open(catalog_db_path)?;
    conn.execute(
        "UPDATE catalog_packages SET commands = ?1 WHERE id = ?2",
        params![
            Some(commands_json.to_string()),
            catalog_package_id(package_name)
        ],
    )?;

    Ok(())
}

fn native_exe_package(name: &str, install_dir: &Path, uninstall_command: String) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: InstallerType::Exe,
        deployment_kind: InstallerType::Exe.deployment_kind(),
        engine_kind: InstallerType::Exe.into(),
        engine_metadata: Some(EngineMetadata::native_exe(Some(uninstall_command), None)),
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies: Vec::new(),
        status: PackageStatus::Ok,
        installed_at: "2026-04-05T00:00:00Z".to_string(),
    }
}

#[test]
fn remove_deletes_portable_installation_and_database_row() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.Remove");
    fs::create_dir_all(&install_dir)?;
    fs::write(install_dir.join("tool.exe"), b"binary")?;

    let package = sample_package(
        "Contoso.Remove",
        InstallerType::Portable,
        &install_dir,
        Vec::new(),
    );

    database::insert_package(&conn, &package)?;

    remove::remove(&package.name, false)?;

    assert!(!install_dir.exists());
    assert!(database::get_package(&conn, &package.name)?.is_none());
    assert!(database::list_packages(&conn)?.is_empty());

    Ok(())
}

#[test]
fn remove_removes_command_shims_after_install() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    let config = init_database(root)?;
    reset_install_state(root)?;
    let resolved_paths = config.resolved_paths();

    let zip_bytes = winbrew_testing::create_dummy_zip_bytes()?;
    let sha512_hash = sha512_hex(&zip_bytes);

    let mut server = MockServer::new();
    let installer_url = format!("{}/shim.zip", server.url());
    let download_mock = server.mock_get("/shim.zip", zip_bytes);

    let catalog_db_dir = root.join("data").join("db");
    fs::create_dir_all(&catalog_db_dir)?;
    seed_catalog_db_with_installer(
        &catalog_db_dir.join("catalog.db"),
        "Winbrew Test Shim",
        "Synthetic package for isolated remove testing",
        &installer_url,
        &sha512_hash,
        InstallerType::Zip,
        None,
    )?;
    seed_catalog_commands(
        &resolved_paths.catalog_db,
        "Winbrew Test Shim",
        r#"["contoso"]"#,
    )?;

    let ctx = AppContext::from_config(&config)?;
    let mut observer = NoopInstallObserver;
    let outcome = install::run(
        &ctx,
        PackageRef::ByName(PackageName::parse("Winbrew Test Shim")?),
        false,
        &mut observer,
    )?;

    let shim_path = std::path::PathBuf::from(&ctx.paths.shims).join("contoso.cmd");
    assert!(shim_path.exists());
    assert!(outcome.result.install_dir.contains("Winbrew Test Shim"));

    remove::remove("Winbrew Test Shim", false)?;

    download_mock.assert();
    assert!(!shim_path.exists());

    let conn = database::get_conn()?;
    assert!(database::get_package(&conn, "Winbrew Test Shim")?.is_none());

    Ok(())
}

#[test]
fn remove_blocks_packages_with_dependents_without_force() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let target_install_dir = root.join("packages").join("Contoso.Target");
    let dependent_install_dir = root.join("packages").join("Contoso.Dependent");

    let target = sample_package(
        "Contoso.Target",
        InstallerType::Portable,
        &target_install_dir,
        Vec::new(),
    );
    let dependent = sample_package(
        "Contoso.Dependent",
        InstallerType::Portable,
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
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let target_install_dir = root.join("packages").join("Contoso.Base");
    let alpha_install_dir = root.join("packages").join("Alpha.Consumer");
    let beta_install_dir = root.join("packages").join("Beta.Consumer");
    let gamma_install_dir = root.join("packages").join("Gamma.Consumer");

    database::insert_package(
        &conn,
        &sample_package(
            "Contoso.Base",
            InstallerType::Portable,
            &target_install_dir,
            Vec::new(),
        ),
    )?;
    database::insert_package(
        &conn,
        &sample_package(
            "Gamma.Consumer",
            InstallerType::Portable,
            &gamma_install_dir,
            vec!["Contoso.Base@1.0.0".to_string()],
        ),
    )?;
    database::insert_package(
        &conn,
        &sample_package(
            "Alpha.Consumer",
            InstallerType::Portable,
            &alpha_install_dir,
            vec!["Contoso.Base@1.0.0".to_string()],
        ),
    )?;
    database::insert_package(
        &conn,
        &sample_package(
            "Beta.Consumer",
            InstallerType::Portable,
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
fn remove_removes_native_exe_package_and_runs_uninstall_command() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.NativeExe");
    fs::create_dir_all(&install_dir)?;
    fs::write(install_dir.join("tool.exe"), b"binary")?;

    let uninstall_marker = root.join("nativeexe-uninstall.log");
    let uninstall_command = format!(
        r#"powershell -NoProfile -Command "Set-Content -LiteralPath '{}' -Value 'ran'""#,
        uninstall_marker.display()
    );
    let package = native_exe_package("Contoso.NativeExe", &install_dir, uninstall_command);

    database::insert_package(&conn, &package)?;

    remove::remove(&package.name, false)?;

    assert!(uninstall_marker.exists());
    assert!(!install_dir.exists());
    assert!(database::get_package(&conn, &package.name)?.is_none());

    Ok(())
}

#[test]
fn remove_removes_native_exe_package_without_uninstall_metadata() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    init_database(root)?;
    reset_install_state(root)?;
    let conn = database::get_conn()?;

    let install_dir = root.join("packages").join("Contoso.NativeExeFallback");
    fs::create_dir_all(&install_dir)?;
    fs::write(install_dir.join("tool.exe"), b"binary")?;

    let package = sample_package(
        "Contoso.NativeExeFallback",
        InstallerType::Exe,
        &install_dir,
        Vec::new(),
    );

    database::insert_package(&conn, &package)?;

    remove::remove(&package.name, false)?;

    assert!(!install_dir.exists());
    assert!(database::get_package(&conn, &package.name)?.is_none());

    Ok(())
}

#[test]
fn remove_removes_font_package_after_install() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();
    let config = init_database(root)?;
    reset_install_state(root)?;

    let font_path = system_font_path()?;
    let font_bytes = fs::read(&font_path)?;
    let sha512_hash = sha512_hex(&font_bytes);
    let font_file_name =
        system_font_file_name(&font_fixture_prefix(root, "winbrew-app-font-remove"))?;

    let mut server = MockServer::new();
    let installer_url = format!("{}/{}", server.url(), font_file_name);
    let download_mock = server.mock_get(&format!("/{}", font_file_name), font_bytes);

    let catalog_db_dir = root.join("data").join("db");
    fs::create_dir_all(&catalog_db_dir)?;
    seed_catalog_db_with_installer(
        &catalog_db_dir.join("catalog.db"),
        "Winbrew Test Font",
        "Synthetic package for isolated remove testing",
        &installer_url,
        &sha512_hash,
        InstallerType::Font,
        None,
    )?;

    let ctx = AppContext::from_config(&config)?;
    let mut observer = NoopInstallObserver;
    let outcome = install::run(
        &ctx,
        PackageRef::ByName(PackageName::parse("Winbrew Test Font")?),
        false,
        &mut observer,
    )?;

    let install_dir = std::path::PathBuf::from(&outcome.result.install_dir);
    assert!(install_dir.exists());

    remove::remove("Winbrew Test Font", false)?;

    download_mock.assert();
    assert!(!install_dir.exists());

    let conn = database::get_conn()?;
    assert!(database::get_package(&conn, "Winbrew Test Font")?.is_none());

    Ok(())
}
