use crate::core::paths::ResolvedPaths;
use crate::models::ConfigSection;
use std::sync::Arc;
use winbrew_ui::UiSettings;

/// Runtime context for the application.
///
/// This contains configuration, paths, UI settings, and logging setup
/// that all commands need to operate.
#[derive(Debug, Clone)]
pub struct AppContext {
    pub ui: UiSettings,
    pub paths: ResolvedPaths,
    pub sections: Vec<ConfigSection>,
    pub root_from_env: bool,
    pub log_level: Arc<str>,
    pub file_log_level: Arc<str>,
}

impl AppContext {
    pub fn from_config(config: &crate::storage::database::Config) -> anyhow::Result<Self> {
        let paths = config.resolved_paths();
        let sections = config.effective_sections()?.into_iter().collect();

        Ok(Self {
            ui: UiSettings {
                color_enabled: config.core.color,
                default_yes: config.core.default_yes,
            },
            paths,
            sections,
            root_from_env: config.env.root_override().is_some(),
            log_level: Arc::from(config.core.log_level.as_str()),
            file_log_level: Arc::from(config.core.file_log_level.as_str()),
        })
    }
}
