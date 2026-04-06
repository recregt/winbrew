#[path = "common/mod.rs"]
mod common;
#[path = "common/env.rs"]
mod test_env;

use common::env_lock;
use std::path::PathBuf;
use test_env::TestEnvVar;
use winbrew::AppContext;
use winbrew::database::{Config, ConfigEnv};
use winbrew::models::config::ConfigSection;
use winbrew::services::{app::doctor::health_report, shared::report::runtime_report};

struct UnsetEnvVar {
    key: &'static str,
    previous: Option<String>,
}

impl UnsetEnvVar {
    fn new(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();

        unsafe {
            std::env::remove_var(key);
        }

        Self { key, previous }
    }
}

impl Drop for UnsetEnvVar {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[test]
fn get_value_returns_current_values() {
    let _guard = env_lock();
    let _root = UnsetEnvVar::new("WINBREW_PATHS_ROOT");
    let config = Config {
        env: ConfigEnv::capture(),
        ..Config::default()
    };

    assert_eq!(
        config.get_value("core.log_level").unwrap(),
        Some("info".to_string())
    );
    assert_eq!(
        config.get_value("paths.root").unwrap(),
        Some(expected_default_root())
    );
}

#[test]
fn removed_network_config_keys_are_rejected() {
    let _guard = env_lock();
    let _root = UnsetEnvVar::new("WINBREW_PATHS_ROOT");
    let mut config = Config {
        env: ConfigEnv::capture(),
        ..Config::default()
    };

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
    let _guard = env_lock();
    let _root = UnsetEnvVar::new("WINBREW_PATHS_ROOT");
    let ctx = app_context(false);
    let report = runtime_report(&ctx).expect("report should build");

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
fn health_report_marks_env_root_source() {
    let _guard = env_lock();
    let root = common::shared_root::test_root();
    let root_path = root.path().to_string_lossy().to_string();
    let _env = TestEnvVar::set("WINBREW_PATHS_ROOT", &root_path);
    common::db::init_database(root.path()).expect("database should initialize");
    std::fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");
    let ctx = app_context(true);
    let report = health_report(&ctx).expect("health report should build");

    assert_eq!(report.install_root_source, "env override");
    assert_eq!(report.install_root, root_path);
    assert_eq!(report.error_count, 0);
    assert!(report.diagnostics.is_empty());
}

fn expected_default_root() -> String {
    PathBuf::from(std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA must be set on Windows"))
        .join("winbrew")
        .to_string_lossy()
        .to_string()
}

fn app_context(root_from_env: bool) -> AppContext {
    let config = Config::load_current().expect("config should load");
    let paths = config.resolved_paths();
    let sections = winbrew::database::config_sections()
        .expect("config sections should load")
        .into_iter()
        .map(|section| ConfigSection {
            title: section.title,
            entries: section.entries,
        })
        .collect();

    AppContext {
        ui: winbrew::ui::UiSettings::default(),
        paths,
        sections,
        root_from_env,
        log_level: config.core.log_level.into(),
        file_log_level: config.core.file_log_level.into(),
    }
}
