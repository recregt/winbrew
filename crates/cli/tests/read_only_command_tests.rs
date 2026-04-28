//! Read-only CLI coverage for listing, search, info, and version commands.
//!
//! The fixture intentionally keeps the database isolated per test so the binary
//! smoke runs can exercise empty and populated states without shared state.

mod common;

use anyhow::Result;
use std::cell::OnceCell;
use std::fs;

use rusqlite::Connection;
use tempfile::TempDir;
use winbrew_app::version;
use winbrew_cli::database::{self};
use winbrew_cli::models::domains::install::InstallerType;
use winbrew_cli::models::domains::installed::PackageStatus;

struct ReadOnlyFixture {
    root: TempDir,
    db_path: std::path::PathBuf,
    db_conn: OnceCell<Connection>,
}

impl ReadOnlyFixture {
    fn new() -> Self {
        let root = common::test_root();
        let config = common::init_database(root.path()).expect("database should initialize");
        fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let resolved_paths = config.resolved_paths();

        Self {
            root,
            db_path: resolved_paths.db,
            db_conn: OnceCell::new(),
        }
    }

    fn package_dir(&self, name: &str) -> std::path::PathBuf {
        self.root.path().join("packages").join(name)
    }

    fn conn(&self) -> &Connection {
        self.db_conn.get_or_init(|| {
            Connection::open(&self.db_path).expect("database connection should open")
        })
    }

    fn insert_package(&self, name: &str) -> Result<()> {
        let install_dir = self.package_dir(name);
        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version("1.2.3")
            .kind(InstallerType::Portable)
            .status(PackageStatus::Ok)
            .build(&install_dir);

        database::insert_package(conn, &package)?;
        Ok(())
    }
}

#[test]
fn read_only_list_reports_when_nothing_is_installed() -> Result<()> {
    let fixture = ReadOnlyFixture::new();

    let list_output = common::run_winbrew(fixture.root.path(), &["list"]);
    common::assert_success(&list_output, "list command")?;
    common::assert_output_contains(&list_output, "No packages are currently installed.")?;

    Ok(())
}

#[test]
fn read_only_commands_cover_cli_views() -> Result<()> {
    let fixture = ReadOnlyFixture::new();
    fixture.insert_package("Contoso App")?;

    let list_output = common::run_winbrew(fixture.root.path(), &["list"]);
    common::assert_success(&list_output, "list command")?;
    common::assert_output_contains_all(
        &list_output,
        &["Contoso App", "Total: 1 package(s) installed."],
    )?;

    let search_output = common::run_winbrew(fixture.root.path(), &["search", "contoso"]);
    common::assert_success(&search_output, "search command")?;
    common::assert_output_contains(
        &search_output,
        "Package catalog not available. Run `winbrew update` first.",
    )?;

    let info_output = common::run_winbrew(fixture.root.path(), &["info"]);
    common::assert_success(&info_output, "info command")?;
    common::assert_output_contains_all(
        &info_output,
        &[
            &format!("WinBrew Package Manager v{}", version::package_version()),
            "Copyright (c) 2026 The WinBrew Contributors.",
            "Licensed under either of MIT or Apache 2.0 at your option.",
            "Windows:",
            "System Architecture:",
            "WinBrew Paths",
            "WinBrew Settings",
            "Database",
            "log_level",
        ],
    )?;
    assert!(!common::output_text(&info_output).contains("Runtime settings displayed."));

    let version_output = common::run_winbrew(fixture.root.path(), &["version"]);
    common::assert_success(&version_output, "version command")?;
    let version_text = String::from_utf8(version_output.stdout)?;
    assert_eq!(version_text.trim(), version::version_string());

    Ok(())
}
