#[path = "common/mod.rs"]
mod common;

use common::{TestEnvVar, env_lock};
use winbrew::database::{Config, ConfigEnv, ConfigSource, get_health_report, get_runtime_report};

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
fn get_value_returns_none_for_unset_optional_fields() {
    let _guard = env_lock();
    let _github_token = UnsetEnvVar::new("WINBREW_CORE_GITHUB_TOKEN");
    let _legacy_github_token = UnsetEnvVar::new("WINBREW_GITHUB_TOKEN");
    let _proxy = UnsetEnvVar::new("WINBREW_CORE_PROXY");
    let _legacy_proxy = UnsetEnvVar::new("WINBREW_PROXY");
    let config = Config {
        env: ConfigEnv::capture(),
        ..Config::default()
    };

    assert_eq!(config.get_value("core.proxy").unwrap(), None);
    assert_eq!(config.get_value("core.github_token").unwrap(), None);
}

#[test]
fn effective_optional_value_returns_none_for_unset_optional_fields() {
    let _guard = env_lock();
    let _github_token = UnsetEnvVar::new("WINBREW_CORE_GITHUB_TOKEN");
    let _legacy_github_token = UnsetEnvVar::new("WINBREW_GITHUB_TOKEN");
    let _proxy = UnsetEnvVar::new("WINBREW_CORE_PROXY");
    let _legacy_proxy = UnsetEnvVar::new("WINBREW_PROXY");
    let config = Config {
        env: ConfigEnv::capture(),
        ..Config::default()
    };

    assert_eq!(config.effective_optional_value("core.proxy").unwrap(), None);
    assert_eq!(
        config
            .effective_optional_value("core.github_token")
            .unwrap(),
        None
    );
}

#[test]
fn effective_optional_value_prefers_env_override() {
    let _guard = env_lock();
    let _env = TestEnvVar::set("WINBREW_CORE_PROXY", "http://localhost:8080");
    let config = Config {
        env: ConfigEnv::capture(),
        ..Config::default()
    };

    assert_eq!(
        config.effective_optional_value("core.proxy").unwrap(),
        Some(("http://localhost:8080".to_string(), ConfigSource::Env,))
    );
}

#[test]
fn runtime_report_builds_expected_sections() {
    let _guard = env_lock();
    let _root = UnsetEnvVar::new("WINBREW_ROOT");
    let _proxy = UnsetEnvVar::new("WINBREW_CORE_PROXY");
    let _token = UnsetEnvVar::new("WINBREW_GITHUB_TOKEN");
    let report = get_runtime_report().expect("report should build");

    assert_eq!(report.sections.len(), 2);
    assert_eq!(report.sections[0].title, "Paths");
    assert_eq!(report.sections[1].title, "Core");
}

#[test]
fn health_report_marks_env_root_source() {
    let _guard = env_lock();
    let _env = TestEnvVar::set("WINBREW_ROOT", r"C:\temp\winbrew");
    let report = get_health_report().expect("health report should build");

    assert_eq!(report.install_root_source, "env override");
    assert_eq!(report.install_root, r"C:\temp\winbrew");
}

#[test]
fn runtime_report_masks_sensitive_and_marks_env_overrides() {
    let _guard = env_lock();
    let _root = TestEnvVar::set("WINBREW_ROOT", r"C:\temp\winbrew");
    let _proxy = TestEnvVar::set("WINBREW_CORE_PROXY", "http://localhost:8080");
    let _token = TestEnvVar::set("WINBREW_GITHUB_TOKEN", "secret-token");

    let report = get_runtime_report().expect("report should build");
    let core = report
        .sections
        .iter()
        .find(|section| section.title == "Core")
        .expect("core section should exist");

    let proxy = core
        .entries
        .iter()
        .find(|(key, _)| key == "proxy")
        .expect("proxy entry should exist");
    assert_eq!(proxy.1, "http://localhost:8080 [env override]");

    let token = core
        .entries
        .iter()
        .find(|(key, _)| key == "github_token")
        .expect("github_token entry should exist");
    assert_eq!(token.1, "(set)");
}
