mod common;

use anyhow::Result;
use std::fs;

use tempfile::TempDir;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::remove as remove_command;
use winbrew_cli::database::{self, Config};
use winbrew_cli::models::domains::install::{EngineKind, InstallerType};
use winbrew_cli::models::domains::installed::{InstalledPackage, PackageStatus};

struct RemoveFixture {
    root: TempDir,
    ctx: CommandContext,
}

impl RemoveFixture {
    fn new() -> Self {
        let root = common::test_root();
        common::init_database(root.path()).expect("database should initialize");
        fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let config = Config::load_at(root.path()).expect("config should load");
        let ctx = CommandContext::from_config(&config).expect("context should build");

        Self { root, ctx }
    }

    fn install_dir(&self, name: &str) -> std::path::PathBuf {
        self.root.path().join("packages").join(name)
    }

    fn insert_portable_package(&self, name: &str) -> std::path::PathBuf {
        let install_dir = self.install_dir(name);
        fs::create_dir_all(&install_dir).expect("install dir should exist");
        fs::write(install_dir.join("tool.txt"), b"payload").expect("install file should exist");

        let conn = database::get_conn().expect("database connection should open");
        let package = InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            engine_kind: EngineKind::Portable,
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        };

        database::insert_package(&conn, &package).expect("package should insert");
        install_dir
    }
}

#[test]
fn remove_removes_portable_package_when_confirmed() -> Result<()> {
    let fixture = RemoveFixture::new();
    let package_name = "Contoso.App";
    let install_dir = fixture.insert_portable_package(package_name);

    remove_command::run(&fixture.ctx, package_name, true, false).expect("remove should succeed");

    assert!(!install_dir.exists());

    let conn = database::get_conn().expect("database connection should open");
    let package = database::get_package(&conn, package_name)?;
    assert!(package.is_none());

    Ok(())
}
