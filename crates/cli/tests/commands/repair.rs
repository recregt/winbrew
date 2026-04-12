#[path = "../common/mod.rs"]
mod common;

use std::path::Path;

use tempfile::TempDir;
use winbrew_cli::AppContext;
use winbrew_cli::app::repair;
use winbrew_cli::database::{self, Config};
use winbrew_cli::models::{EngineKind, InstalledPackage, InstallerType, PackageStatus};

struct RepairFixture {
    root: TempDir,
    ctx: AppContext,
}

impl RepairFixture {
    fn new() -> Self {
        let root = common::test_root();
        common::init_database(root.path()).expect("database should initialize");
        std::fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let config = Config::load_at(root.path()).expect("config should load");
        let ctx = AppContext::from_config(&config).expect("context should build");

        Self { root, ctx }
    }

    fn root_path(&self) -> &Path {
        self.root.path()
    }

    fn insert_stale_package(&self, name: &str) {
        let conn = database::get_conn().expect("database connection should open");
        let package = InstalledPackage {
            name: name.to_string(),
            version: "0.9.0".to_string(),
            kind: InstallerType::Portable,
            engine_kind: EngineKind::Portable,
            engine_metadata: None,
            install_dir: self
                .root_path()
                .join("packages")
                .join(name)
                .to_string_lossy()
                .to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Installing,
            installed_at: "2026-04-01T00:00:00Z".to_string(),
        };

        database::insert_package(&conn, &package).expect("package should insert");
    }
}

#[test]
fn repair_replays_committed_journal_into_database() {
    let fixture = RepairFixture::new();
    let package_name = "winget/Contoso.App";
    let journal_install_dir = fixture.root_path().join("packages").join("Contoso.App");
    fixture.insert_stale_package(package_name);

    let mut writer =
        database::JournalWriter::open_for_package(fixture.root_path(), package_name, "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: "1.0.0".to_string(),
            engine: "portable".to_string(),
            install_dir: journal_install_dir.to_string_lossy().to_string(),
            dependencies: vec!["winget/Contoso.Dependency".to_string()],
            engine_metadata: None,
        })
        .expect("write metadata");
    writer
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    let conn = database::get_conn().expect("database connection should open");
    let package = database::get_package(&conn, package_name)
        .expect("read package")
        .expect("package should exist");

    assert_eq!(package.version, "1.0.0");
    assert_eq!(package.kind, InstallerType::Portable);
    assert_eq!(package.engine_kind, EngineKind::Portable);
    assert_eq!(
        package.install_dir,
        journal_install_dir.to_string_lossy().to_string()
    );
    assert_eq!(
        package.dependencies,
        vec!["winget/Contoso.Dependency".to_string()]
    );
    assert_eq!(package.status, PackageStatus::Ok);
    assert_eq!(package.installed_at, "2026-04-12T00:00:00Z");
}

#[test]
fn repair_removes_orphan_install_directory() {
    let fixture = RepairFixture::new();
    let orphan_dir = fixture.root_path().join("packages").join("Contoso.Orphan");
    std::fs::create_dir_all(&orphan_dir).expect("orphan dir should exist");

    assert!(orphan_dir.exists());

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    assert!(!orphan_dir.exists());
}
