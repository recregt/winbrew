use crate::core::paths::ResolvedPaths;
use crate::models::domains::shared::ConfigSection;
use std::sync::Arc;

/// Runtime context for the application.
///
/// This contains configuration, paths, and logging setup that all commands
/// need to operate.
#[derive(Debug, Clone)]
pub struct AppContext {
    pub paths: ResolvedPaths,
    pub sections: Vec<ConfigSection>,
    pub root_from_env: bool,
    pub log_level: Arc<str>,
    pub file_log_level: Arc<str>,
    pub verbosity: u8,
}

impl AppContext {
    pub fn from_config(config: &crate::database::Config) -> anyhow::Result<Self> {
        Self::from_config_with_verbosity(config, 0)
    }

    pub fn from_config_with_verbosity(
        config: &crate::database::Config,
        verbosity: u8,
    ) -> anyhow::Result<Self> {
        let paths = config.resolved_paths();
        let sections = config.effective_sections()?.into_iter().collect();

        Ok(Self {
            paths,
            sections,
            root_from_env: config.env.root_override().is_some(),
            log_level: Arc::from(config.core.log_level.as_str()),
            file_log_level: Arc::from(config.core.file_log_level.as_str()),
            verbosity,
        })
    }
}
