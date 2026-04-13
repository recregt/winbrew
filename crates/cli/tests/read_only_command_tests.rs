mod common;

use std::fs;

use tempfile::TempDir;
use winbrew_app::version;
use winbrew_cli::database;
use winbrew_cli::models::domains::install::{EngineKind, InstallerType};
use winbrew_cli::models::domains::installed::{InstalledPackage, PackageStatus};

struct ReadOnlyFixture {
    root: TempDir,
}

impl ReadOnlyFixture {
    fn new() -> Self {
        let root = common::test_root();
        common::init_database(root.path()).expect("database should initialize");
        fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        Self { root }
    }

    fn insert_package(&self, name: &str) {
        let conn = database::get_conn().expect("database connection should open");
        let package = InstalledPackage {
            name: name.to_string(),
            version: "1.2.3".to_string(),
            kind: InstallerType::Portable,
            engine_kind: EngineKind::Portable,
            engine_metadata: None,
            install_dir: self
                .root
                .path()
                .join("packages")
                .join(name)
                .to_string_lossy()
                .to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        };

        database::insert_package(&conn, &package).expect("package should insert");
    }
}

fn output_text(output: &std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

#[test]
fn read_only_commands_cover_cli_views() {
    let fixture = ReadOnlyFixture::new();
    fixture.insert_package("Contoso App");

    let list_output = common::run_winbrew(fixture.root.path(), &["list"]);
    assert!(list_output.status.success(), "list should succeed");
    let list_text = output_text(&list_output);
    assert!(list_text.contains("Contoso App"));
    assert!(list_text.contains("Total: 1 package(s) installed."));

    let search_output = common::run_winbrew(fixture.root.path(), &["search", "contoso"]);
    assert!(search_output.status.success(), "search should succeed");
    let search_text = output_text(&search_output);
    assert!(search_text.contains("Package catalog not available. Run `winbrew update` first."));

    let info_output = common::run_winbrew(fixture.root.path(), &["info"]);
    assert!(info_output.status.success(), "info should succeed");
    let info_text = output_text(&info_output);
    assert!(info_text.contains("Version:"));
    assert!(info_text.contains("Runtime settings displayed."));

    let version_output = common::run_winbrew(fixture.root.path(), &["version"]);
    assert!(version_output.status.success(), "version should succeed");
    let version_text = String::from_utf8(version_output.stdout).expect("stdout should be utf-8");
    assert_eq!(version_text.trim(), version::version_string());
}
