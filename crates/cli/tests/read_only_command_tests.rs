mod common;

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
        }
    }

    fn conn(&self) -> Connection {
        Connection::open(&self.db_path).expect("database connection should open")
    }

    fn insert_package(&self, name: &str) {
        let install_dir = self.root.path().join("packages").join(name);
        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version("1.2.3")
            .kind(InstallerType::Portable)
            .status(PackageStatus::Ok)
            .build(&install_dir);

        database::insert_package(&conn, &package).expect("package should insert");
    }
}

#[test]
fn read_only_commands_cover_cli_views() {
    let fixture = ReadOnlyFixture::new();
    fixture.insert_package("Contoso App");

    let list_output = common::run_winbrew(fixture.root.path(), &["list"]);
    common::assert_success(&list_output, "list command");
    common::assert_output_contains_all(
        &list_output,
        &["Contoso App", "Total: 1 package(s) installed."],
    );

    let search_output = common::run_winbrew(fixture.root.path(), &["search", "contoso"]);
    common::assert_success(&search_output, "search command");
    common::assert_output_contains(
        &search_output,
        "Package catalog not available. Run `winbrew update` first.",
    );

    let info_output = common::run_winbrew(fixture.root.path(), &["info"]);
    common::assert_success(&info_output, "info command");
    common::assert_output_contains_all(&info_output, &["Version:", "Runtime settings displayed."]);

    let version_output = common::run_winbrew(fixture.root.path(), &["version"]);
    common::assert_success(&version_output, "version command");
    let version_text = String::from_utf8(version_output.stdout).expect("stdout should be utf-8");
    assert_eq!(version_text.trim(), version::version_string());
}
