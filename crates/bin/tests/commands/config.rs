#[path = "../common/mod.rs"]
mod common;

use std::path::Path;

use tempfile::TempDir;
use winbrew::AppContext;
use winbrew::app::doctor::health_report;
use winbrew::app::report::runtime_report;
use winbrew::cli::ConfigCommand;
use winbrew::commands::{config as config_command, error::CommandError};
use winbrew::database::Config;

struct ConfigFixture {
    root: TempDir,
    config: Config,
    ctx: AppContext,
}

impl ConfigFixture {
    fn new() -> Self {
        let root = common::test_root();
        common::init_database(root.path()).expect("database should initialize");
        std::fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let config = Config::load_at(root.path()).expect("config should load");
        let ctx = AppContext::from_config(&config).expect("context should build");

        Self { root, config, ctx }
    }

    fn root_path(&self) -> &Path {
        self.root.path()
    }
}

#[test]
fn get_value_returns_current_values() {
    let config = Config::default();
    let default_root = Config::default().paths.root;

    assert_eq!(
        config.get_value("core.log_level").unwrap(),
        Some("info".to_string())
    );
    assert_eq!(config.get_value("paths.root").unwrap(), Some(default_root));
}

#[test]
fn removed_network_config_keys_are_rejected() {
    let mut config = Config::default();

    for key in [
        "core.download_timeout",
        "core.concurrent_downloads",
        "core.proxy",
        "core.github_token",
    ] {
        assert!(config.get_value(key).is_err(), "{key} should be removed");
        assert!(
            config.set_value(key, "value").is_err(),
            "{key} should not be settable"
        );
    }
}

#[test]
fn runtime_report_builds_expected_sections() {
    let config = Config::default();
    let ctx = AppContext::from_config(&config).expect("context should build");
    let report = runtime_report(&ctx.sections, &ctx.paths).expect("report should build");

    assert_eq!(report.sections.len(), 2);
    assert_eq!(report.sections[0].title, "Paths");
    assert_eq!(report.sections[1].title, "Core");

    let core = report
        .sections
        .iter()
        .find(|section| section.title == "Core")
        .expect("core section should exist");

    let core_keys: Vec<&str> = core.entries.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        core_keys,
        vec![
            "log_level",
            "file_log_level",
            "auto_update",
            "confirm_remove",
            "default_yes",
            "color",
        ]
    );
}

#[test]
fn health_report_uses_config_root_source() {
    let fixture = ConfigFixture::new();
    let report = health_report(&fixture.ctx).expect("health report should build");

    assert_eq!(report.install_root_source, "config:paths.root");
    assert_eq!(
        report.install_root,
        fixture.root_path().to_string_lossy().to_string()
    );
    assert_eq!(report.error_count, 0);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn config_set_rejects_empty_values() {
    let mut fixture = ConfigFixture::new();
    let err = config_command::run(
        &fixture.ctx,
        &mut fixture.config,
        ConfigCommand::Set {
            key: "core.log_level".to_string(),
            value: Some("   ".to_string()),
        },
    )
    .expect_err("empty value should fail");

    let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
    assert!(matches!(cmd_err, CommandError::Reported { .. }));
    assert_eq!(cmd_err.exit_code(), 1);
}

#[test]
fn config_set_accepts_valid_value() {
    let mut fixture = ConfigFixture::new();

    config_command::run(
        &fixture.ctx,
        &mut fixture.config,
        ConfigCommand::Set {
            key: "core.log_level".to_string(),
            value: Some("debug".to_string()),
        },
    )
    .expect("valid value should succeed");

    let config = Config::load_at(fixture.root_path()).expect("config should load");

    assert_eq!(config.core.log_level, "debug");
}
