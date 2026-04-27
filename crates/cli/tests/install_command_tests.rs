//! Tests for the install command's argument validation and early failures.

mod common;

use std::fs;

use tempfile::TempDir;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::install as install_command;
use winbrew_cli::database;

struct InstallFixture {
    root: TempDir,
    ctx: CommandContext,
}

impl InstallFixture {
    fn new() -> Self {
        let root = common::test_root();
        let config = common::init_database(root.path()).expect("database should initialize");
        fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let ctx = CommandContext::from_config(&config).expect("context should build");

        Self { root, ctx }
    }
}

#[test]
fn install_rejects_empty_query() {
    let fixture = InstallFixture::new();
    let err =
        install_command::run(&fixture.ctx, &[], false, false).expect_err("empty query should fail");

    assert_eq!(err.to_string(), "package query cannot be empty");
    assert!(fixture.root.path().join("packages").exists());
}

#[test]
fn install_rejects_invalid_package_reference() {
    let fixture = InstallFixture::new();
    let query = vec!["@invalid".to_string()];

    let err = install_command::run(&fixture.ctx, &query, false, false)
        .expect_err("invalid package reference should fail");
    let text = err.to_string();

    assert!(text.contains("invalid package id invalid"));
    assert!(text.contains(
        "expected @winget/<id>, @scoop/<bucket>/<id>, @chocolatey/<id>, or @winbrew/<id>"
    ));
}

#[test]
fn install_plan_mode_is_read_only() -> anyhow::Result<()> {
    let fixture = InstallFixture::new();
    let catalog_db = fixture
        .root
        .path()
        .join("data")
        .join("db")
        .join("catalog.db");

    fs::create_dir_all(catalog_db.parent().expect("catalog db parent"))?;

    common::seed_catalog_db_with_installer(
        &catalog_db,
        "Winbrew Test Zip",
        "Synthetic package for install plan testing",
        "https://example.invalid/test.zip",
        &common::sha512_hex(b"unused"),
        winbrew_cli::models::domains::install::InstallerType::Zip,
        None,
    )?;

    let query = vec!["Winbrew".to_string(), "Test".to_string(), "Zip".to_string()];
    install_command::run(&fixture.ctx, &query, false, true)?;

    assert!(
        !fixture
            .root
            .path()
            .join("packages")
            .join("Winbrew Test Zip")
            .exists()
    );

    let conn = database::get_conn()?;
    assert!(database::get_package(&conn, "Winbrew Test Zip")?.is_none());

    Ok(())
}

#[test]
fn install_plan_mode_shortens_url_and_hides_temp_root() -> anyhow::Result<()> {
    let fixture = InstallFixture::new();
    let catalog_db = fixture
        .root
        .path()
        .join("data")
        .join("db")
        .join("catalog.db");

    fs::create_dir_all(catalog_db.parent().expect("catalog db parent"))?;

    common::seed_catalog_db_with_installer(
        &catalog_db,
        "Winbrew Claude Code",
        "Synthetic package for install plan output testing",
        "https://storage.googleapis.com/winbrew-downloads/releases/2026/04/claude.exe",
        &common::sha512_hex(b"unused"),
        winbrew_cli::models::domains::install::InstallerType::Zip,
        None,
    )?;

    let output = common::run_winbrew(
        fixture.root.path(),
        &["install", "Winbrew", "Claude", "Code", "--plan"],
    );
    common::assert_success(&output, "install plan mode")?;

    let text = common::output_text(&output);
    assert!(text.contains("Installer URL: storage.googleapis.com/.../claude.exe"));
    assert!(text.contains("Engine: zip"));
    assert!(text.contains("Deployment: portable"));
    assert!(!text.contains("Temp root:"));

    let verbose_output = common::run_winbrew(
        fixture.root.path(),
        &["-v", "install", "Winbrew", "Claude", "Code", "--plan"],
    );
    common::assert_success(&verbose_output, "verbose install plan mode")?;

    let verbose_text = common::output_text(&verbose_output);
    assert!(verbose_text.contains("Temp root:"));

    Ok(())
}
