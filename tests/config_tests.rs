#[path = "common/mod.rs"]
mod common;

use common::{TestEnvVar, env_lock};
use winbrew::database::{Config, ConfigEnv, get_health_report, get_runtime_report};

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
    let _root = UnsetEnvVar::new("WINBREW_ROOT");
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
        Some(r"C:\winbrew".to_string())
    );
}

#[test]
fn removed_network_config_keys_are_rejected() {
    let _guard = env_lock();
    let _root = UnsetEnvVar::new("WINBREW_ROOT");
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
    let _root = UnsetEnvVar::new("WINBREW_ROOT");
    let report = get_runtime_report().expect("report should build");

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
    let _env = TestEnvVar::set("WINBREW_ROOT", r"C:\temp\winbrew");
    let report = get_health_report().expect("health report should build");

    assert_eq!(report.install_root_source, "env override");
    assert_eq!(report.install_root, r"C:\temp\winbrew");
}
