use anyhow::Result;

use crate::core::paths::ResolvedPaths;
use winbrew_models::{ConfigSection, ReportSection, RuntimeReport};

pub fn runtime_report(
    sections: &[ConfigSection],
    resolved_paths: &ResolvedPaths,
) -> Result<RuntimeReport> {
    let core_section = section(sections, "Core")?;

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
