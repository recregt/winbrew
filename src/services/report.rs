use anyhow::Result;

use crate::AppContext;
use crate::services::config::ConfigSection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    pub database_path: String,
    pub database_exists: bool,
    pub catalog_database_path: String,
    pub catalog_database_exists: bool,
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
                "Catalog database".to_string(),
                self.catalog_database_path.clone(),
            ),
            (
                "Catalog database exists".to_string(),
                yes_no(self.catalog_database_exists),
            ),
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

pub fn health_report(ctx: &AppContext) -> Result<HealthReport> {
    let resolved_paths = &ctx.paths;

    Ok(HealthReport {
        database_path: resolved_paths.db.to_string_lossy().to_string(),
        database_exists: resolved_paths.db.exists(),
        catalog_database_path: resolved_paths.catalog_db.to_string_lossy().to_string(),
        catalog_database_exists: resolved_paths.catalog_db.exists(),
        install_root_source: if ctx.root_from_env {
            "env override".to_string()
        } else {
            "config:paths.root".to_string()
        },
        install_root: resolved_paths.root.to_string_lossy().to_string(),
        install_root_exists: resolved_paths.root.exists(),
        packages_dir: resolved_paths.packages.to_string_lossy().to_string(),
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

pub fn runtime_report(ctx: &AppContext) -> Result<RuntimeReport> {
    let sections = &ctx.sections;
    let core_section = section(sections, "Core")?;
    let resolved_paths = &ctx.paths;

    let sections = vec![
        ReportSection {
            title: "Paths".to_string(),
            entries: vec![
                (
                    "Database".to_string(),
                    resolved_paths.db.to_string_lossy().to_string(),
                ),
                (
                    "Catalog DB".to_string(),
                    resolved_paths.catalog_db.to_string_lossy().to_string(),
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
        render_section(core_section),
    ];

    Ok(RuntimeReport::new(sections))
}

fn section<'a>(sections: &'a [ConfigSection], title: &str) -> Result<&'a ConfigSection> {
    sections
        .iter()
        .find(|section| section.title == title)
        .ok_or_else(|| anyhow::anyhow!("missing config section: {title}"))
}

fn render_section(section: &ConfigSection) -> ReportSection {
    let mut entries = Vec::with_capacity(section.entries.len());

    for (key, file_value) in &section.entries {
        entries.push((key.clone(), file_value.clone()));
    }

    ReportSection {
        title: section.title.clone(),
        entries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_report_to_kv_formats_boolean_flags() {
        let report = HealthReport {
            database_path: "db.sqlite".to_string(),
            database_exists: true,
            catalog_database_path: "catalog.sqlite".to_string(),
            catalog_database_exists: false,
            install_root_source: "env override".to_string(),
            install_root: "C:\\winbrew".to_string(),
            install_root_exists: true,
            packages_dir: "C:\\winbrew\\packages".to_string(),
        };

        let entries = report.to_kv();

        assert_eq!(
            entries,
            vec![
                ("Database".to_string(), "db.sqlite".to_string()),
                ("Database exists".to_string(), "yes".to_string()),
                ("Catalog database".to_string(), "catalog.sqlite".to_string()),
                ("Catalog database exists".to_string(), "no".to_string()),
                (
                    "Install root source".to_string(),
                    "env override".to_string()
                ),
                ("Install root".to_string(), "C:\\winbrew".to_string()),
                ("Install root exists".to_string(), "yes".to_string()),
                (
                    "Packages dir".to_string(),
                    "C:\\winbrew\\packages".to_string()
                ),
            ]
        );
    }
}
