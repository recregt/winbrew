use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::core::paths;
use crate::database::ConfigSource;

use super::{Config, ConfigSection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    pub database_path: String,
    pub database_exists: bool,
    pub install_root_source: String,
    pub install_root: String,
    pub install_root_exists: bool,
    pub packages_dir: String,
}

impl HealthReport {
    pub fn to_kv(&self) -> Vec<(String, String)> {
        vec![
            ("Database".to_string(), self.database_path.clone()),
            ("Database exists".to_string(), yes_no(self.database_exists)),
            (
                "Install root source".to_string(),
                self.install_root_source.clone(),
            ),
            ("Install root".to_string(), self.install_root.clone()),
            (
                "Install root exists".to_string(),
                yes_no(self.install_root_exists),
            ),
            ("Packages dir".to_string(), self.packages_dir.clone()),
        ]
    }
}

pub fn get_health_report() -> Result<HealthReport> {
    let config = Config::current();
    let (root, source) = config.effective_value("paths.root")?;
    let paths = paths::resolved_paths(
        Path::new(&root),
        &config.paths.packages,
        &config.paths.data,
        &config.paths.logs,
        &config.paths.cache,
    );
    Ok(HealthReport {
        database_path: paths::db_path().to_string_lossy().to_string(),
        database_exists: paths::db_path().exists(),
        install_root_source: match source {
            ConfigSource::Env => "env override".to_string(),
            ConfigSource::File => "config:paths.root".to_string(),
        },
        install_root: paths.root.to_string_lossy().to_string(),
        install_root_exists: paths.root.exists(),
        packages_dir: paths.packages.to_string_lossy().to_string(),
    })
}

fn yes_no(value: bool) -> String {
    if value {
        "yes".to_string()
    } else {
        "no".to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub sections: Vec<ReportSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSection {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

impl RuntimeReport {
    pub fn new(sections: Vec<ReportSection>) -> Self {
        Self { sections }
    }
}

pub fn get_runtime_report() -> Result<RuntimeReport> {
    build_runtime_report(&Config::current())
}

fn build_runtime_report(config: &Config) -> Result<RuntimeReport> {
    let sections = config.sections();
    let paths_section = section(&sections, "Paths")?;
    let core_section = section(&sections, "Core")?;
    let sources_section = section(&sections, "Sources")?;

    let path_values = effective_values(config, paths_section)?;
    let resolved_paths = paths::resolved_paths(
        std::path::Path::new(&path_values["root"]),
        &path_values["packages"],
        &path_values["data"],
        &path_values["logs"],
        &path_values["cache"],
    );

    let sections = vec![
        ReportSection {
            title: "Paths".to_string(),
            entries: vec![
                (
                    "Database".to_string(),
                    resolved_paths.db.to_string_lossy().to_string(),
                ),
                (
                    "Config file".to_string(),
                    resolved_paths.config.to_string_lossy().to_string(),
                ),
                (
                    "Log file".to_string(),
                    resolved_paths.log.to_string_lossy().to_string(),
                ),
                (
                    "Install root".to_string(),
                    resolved_paths.root.to_string_lossy().to_string(),
                ),
                (
                    "Packages dir".to_string(),
                    resolved_paths.packages.to_string_lossy().to_string(),
                ),
                (
                    "Cache dir".to_string(),
                    resolved_paths.cache.to_string_lossy().to_string(),
                ),
            ],
        },
        render_section(config, core_section)?,
        render_section(config, sources_section)?,
    ];

    Ok(RuntimeReport::new(sections))
}

fn section<'a>(sections: &'a [ConfigSection], title: &str) -> Result<&'a ConfigSection> {
    sections
        .iter()
        .find(|section| section.title == title)
        .ok_or_else(|| anyhow::anyhow!("missing config section: {title}"))
}

fn effective_values(config: &Config, section: &ConfigSection) -> Result<HashMap<String, String>> {
    let mut values = HashMap::with_capacity(section.entries.len());

    for (key, _) in &section.entries {
        let full_key = crate::database::config::section_key(&section.title, key);
        let (value, _) = config.effective_value(&full_key)?;
        values.insert(key.clone(), value);
    }

    Ok(values)
}

fn render_section(config: &Config, section: &ConfigSection) -> Result<ReportSection> {
    let mut entries = Vec::with_capacity(section.entries.len());

    for (key, file_value) in &section.entries {
        let full_key = crate::database::config::section_key(&section.title, key);
        let value = match full_key.as_str() {
            "core.proxy" => match config.effective_optional_value(&full_key)? {
                Some((value, source)) => render_optional_value(value, source, "(none)"),
                None => "(none)".to_string(),
            },
            "core.github_token" => match config.effective_optional_value(&full_key)? {
                Some((value, source)) => render_sensitive_value(value, source, "(unset)"),
                None => "(unset)".to_string(),
            },
            "core.download_timeout" => {
                let (value, _) = config.effective_value(&full_key)?;
                format!("{value}s")
            }
            _ => config
                .effective_value(&full_key)
                .map(|(value, _)| value)
                .unwrap_or_else(|_| file_value.clone()),
        };

        entries.push((key.clone(), value));
    }

    Ok(ReportSection {
        title: section.title.clone(),
        entries,
    })
}

fn render_optional_value(
    value: String,
    source: ConfigSource,
    empty_label: &str,
) -> String {
    if value.trim().is_empty() {
        empty_label.to_string()
    } else if matches!(source, ConfigSource::Env) {
        format!("{value} [env override]")
    } else {
        value
    }
}

fn render_sensitive_value(
    value: String,
    _source: ConfigSource,
    empty_label: &str,
) -> String {
    if value.trim().is_empty() {
        empty_label.to_string()
    } else {
        "(set)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap()
    }

    struct TestEnvVar {
        key: &'static str,
    }

    impl TestEnvVar {
        fn set(key: &'static str, value: &str) -> Self {
            unsafe {
                std::env::set_var(key, value);
            }

            Self { key }
        }
    }

    impl Drop for TestEnvVar {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn runtime_report_builds_expected_sections() {
        let report = build_runtime_report(&Config::default()).expect("report should build");

        assert_eq!(report.sections.len(), 3);
        assert_eq!(report.sections[0].title, "Paths");
        assert_eq!(report.sections[1].title, "Core");
        assert_eq!(report.sections[2].title, "Sources");
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

        let report = build_runtime_report(&Config::default()).expect("report should build");
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

    #[test]
    fn optional_and_sensitive_helpers_format_values() {
        assert_eq!(
            render_optional_value("http://localhost:8080".to_string(), ConfigSource::Env, "(none)"),
            "http://localhost:8080 [env override]"
        );
        assert_eq!(
            render_optional_value("".to_string(), ConfigSource::File, "(none)"),
            "(none)"
        );
        assert_eq!(
            render_sensitive_value("secret-token".to_string(), ConfigSource::Env, "(unset)"),
            "(set)"
        );
        assert_eq!(
            render_sensitive_value("".to_string(), ConfigSource::File, "(unset)"),
            "(unset)"
        );
    }
}
