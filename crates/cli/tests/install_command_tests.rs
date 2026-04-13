mod common;

use std::fs;

use tempfile::TempDir;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::install as install_command;

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
    let err = install_command::run(&fixture.ctx, &[], false).expect_err("empty query should fail");

    assert_eq!(err.to_string(), "package query cannot be empty");
    assert!(fixture.root.path().join("packages").exists());
}
