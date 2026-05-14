//! Remove command coverage for confirmed removals.

mod common;

use anyhow::Result;
use std::cell::OnceCell;
use std::fs;

use rusqlite::Connection;
use tempfile::TempDir;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::remove as remove_command;
use winbrew_cli::database::{self};
use winbrew_cli::models::domains::install::{EngineMetadata, InstallerType};
use winbrew_cli::models::domains::installed::PackageStatus;

struct RemoveFixture {
    root: TempDir,
    db_path: std::path::PathBuf,
    db_conn: OnceCell<Connection>,
    ctx: CommandContext,
}

impl RemoveFixture {
    fn new() -> Self {
        let root = common::test_root();
        let config = common::init_database(root.path()).expect("database should initialize");
        fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let resolved_paths = config.resolved_paths();
        let ctx = CommandContext::from_config(&config).expect("context should build");

        Self {
            root,
            db_path: resolved_paths.db,
            db_conn: OnceCell::new(),
            ctx,
        }
    }

    fn install_dir(&self, name: &str) -> std::path::PathBuf {
        self.root.path().join("packages").join(name)
    }

    fn conn(&self) -> &Connection {
        self.db_conn.get_or_init(|| {
            Connection::open(&self.db_path).expect("database connection should open")
        })
    }

    fn insert_portable_package(&self, name: &str) -> Result<std::path::PathBuf> {
        let install_dir = self.install_dir(name);
        fs::create_dir_all(&install_dir).expect("install dir should exist");
        fs::write(install_dir.join("tool.txt"), b"payload").expect("install file should exist");

        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version("1.0.0")
            .kind(InstallerType::Portable)
            .status(PackageStatus::Ok)
            .build(&install_dir);

        database::insert_package(conn, &package)?;
        Ok(install_dir)
    }

    fn insert_native_exe_package(
        &self,
        name: &str,
        uninstall_command: String,
    ) -> Result<std::path::PathBuf> {
        let install_dir = self.install_dir(name);
        fs::create_dir_all(&install_dir).expect("install dir should exist");
        fs::write(install_dir.join("tool.exe"), b"payload").expect("install file should exist");

        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version("1.0.0")
            .kind(InstallerType::Exe)
            .engine_metadata(Some(EngineMetadata::native_exe(
                Some(uninstall_command),
                None,
            )))
            .status(PackageStatus::Ok)
            .build(&install_dir);

        database::insert_package(conn, &package)?;
        Ok(install_dir)
    }
}

#[test]
fn remove_removes_portable_package_when_confirmed() -> Result<()> {
    let fixture = RemoveFixture::new();
    let package_name = "Contoso.App";
    let install_dir = fixture.insert_portable_package(package_name)?;

    remove_command::run(&fixture.ctx, package_name, true, false).expect("remove should succeed");

    anyhow::ensure!(!install_dir.exists(), "install directory should be removed");

    let conn = fixture.conn();
    let package = database::get_package(conn, package_name)?;
    anyhow::ensure!(package.is_none(), "package should be removed from database");

    Ok(())
}

#[test]
fn remove_removes_native_exe_package_when_confirmed() -> Result<()> {
    let fixture = RemoveFixture::new();
    let package_name = "Contoso.NativeExe";
    let uninstall_marker = fixture.root.path().join("nativeexe-uninstall.log");
    let uninstall_command = format!(
        r#"powershell -NoProfile -Command "Set-Content -LiteralPath '{}' -Value 'ran'""#,
        uninstall_marker.display()
    );
    let install_dir = fixture.insert_native_exe_package(package_name, uninstall_command)?;

    remove_command::run(&fixture.ctx, package_name, true, false).expect("remove should succeed");

    anyhow::ensure!(uninstall_marker.exists(), "uninstall command should run");
    anyhow::ensure!(!install_dir.exists(), "install directory should be removed");

    let conn = fixture.conn();
    let package = database::get_package(conn, package_name)?;
    anyhow::ensure!(package.is_none(), "package should be removed from database");

    Ok(())
}
