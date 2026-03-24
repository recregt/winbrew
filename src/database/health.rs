use anyhow::Result;

use crate::core::paths;

use super::Config;

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
    let paths = config.resolved_paths();

    Ok(HealthReport {
        database_path: paths::db_path().to_string_lossy().to_string(),
        database_exists: paths::db_path().exists(),
        install_root_source: "config:paths.root".to_string(),
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
    let root = effective_path_value(config, "paths.root")?;
    let packages = effective_path_value(config, "paths.packages")?;
    let data = effective_path_value(config, "paths.data")?;
    let logs = effective_path_value(config, "paths.logs")?;
    let cache = effective_path_value(config, "paths.cache")?;

    let resolved_paths =
        paths::resolved_paths(std::path::Path::new(&root), &packages, &data, &logs, &cache);

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
        ReportSection {
            title: "Core".to_string(),
            entries: vec![
                (
                    "log_level".to_string(),
                    effective_string(config, "core.log_level")?,
                ),
                (
                    "file_log_level".to_string(),
                    effective_string(config, "core.file_log_level")?,
                ),
                (
                    "auto_update".to_string(),
                    effective_string(config, "core.auto_update")?,
                ),
                (
                    "confirm_remove".to_string(),
                    effective_string(config, "core.confirm_remove")?,
                ),
                (
                    "default_yes".to_string(),
                    effective_string(config, "core.default_yes")?,
                ),
                ("color".to_string(), effective_string(config, "core.color")?),
                (
                    "download_timeout".to_string(),
                    format!("{}s", effective_string(config, "core.download_timeout")?),
                ),
                (
                    "concurrent_downloads".to_string(),
                    effective_string(config, "core.concurrent_downloads")?,
                ),
                (
                    "proxy".to_string(),
                    mask_optional(config, "core.proxy", "(none)")?,
                ),
                (
                    "github_token".to_string(),
                    mask_optional(config, "core.github_token", "(unset)")?,
                ),
            ],
        },
        ReportSection {
            title: "Sources".to_string(),
            entries: vec![
                (
                    "primary".to_string(),
                    effective_string(config, "sources.primary")?,
                ),
                (
                    "winget.url".to_string(),
                    effective_string(config, "sources.winget.url")?,
                ),
                (
                    "winget.format".to_string(),
                    effective_string(config, "sources.winget.format")?,
                ),
                (
                    "winget.manifest_kind".to_string(),
                    effective_string(config, "sources.winget.manifest_kind")?,
                ),
                (
                    "winget.manifest_path_template".to_string(),
                    effective_string(config, "sources.winget.manifest_path_template")?,
                ),
                (
                    "winget.enabled".to_string(),
                    effective_string(config, "sources.winget.enabled")?,
                ),
            ],
        },
    ];

    Ok(RuntimeReport::new(sections))
}

fn effective_string(config: &Config, key: &str) -> Result<String> {
    config.effective_value(key).map(|(value, _)| value)
}

fn mask_optional(config: &Config, key: &str, empty_label: &str) -> Result<String> {
    Ok(match config.effective_optional_value(key)? {
        Some((value, "env")) => {
            if value.trim().is_empty() {
                empty_label.to_string()
            } else if key == "core.github_token" {
                "(set)".to_string()
            } else {
                format!("{value} [env override]")
            }
        }
        Some((value, _)) if value.trim().is_empty() => empty_label.to_string(),
        Some((_, _)) if key == "core.github_token" => "(set)".to_string(),
        Some((value, _)) => value,
        None => empty_label.to_string(),
    })
}

fn effective_path_value(config: &Config, key: &str) -> Result<String> {
    effective_string(config, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_report_builds_expected_sections() {
        let report = build_runtime_report(&Config::default()).expect("report should build");

        assert_eq!(report.sections.len(), 3);
        assert_eq!(report.sections[0].title, "Paths");
        assert_eq!(report.sections[1].title, "Core");
        assert_eq!(report.sections[2].title, "Sources");
    }
}
