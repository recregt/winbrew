#![cfg(windows)]

use anyhow::Result;
use std::sync::Arc;

pub mod cli;
pub mod commands;
pub mod services;

use crate::commands::run;
use crate::core::paths::ResolvedPaths;
use crate::services::bootstrap;
use crate::services::shared::config as shared_config;
pub use winbrew_install::storage as database;
pub use winbrew_install::{cancel, catalog, core, engines, models};
pub use winbrew_ui::{Ui, UiSettings};

#[derive(Debug, Clone)]
pub struct AppContext {
    pub ui: UiSettings,
    pub paths: ResolvedPaths,
    pub sections: Vec<models::ConfigSection>,
    pub root_from_env: bool,
    pub log_level: Arc<str>,
    pub file_log_level: Arc<str>,
}

impl AppContext {
    pub fn from_config(config: &crate::database::Config) -> Result<Self> {
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

pub fn run_app(command: crate::cli::Command) -> Result<()> {
    let mut config = shared_config::load_current()?;
    let ctx = AppContext::from_config(&config)?;

    bootstrap::logging::init(&ctx.paths.logs, &ctx.log_level, &ctx.file_log_level)?;
    database::init(&ctx.paths)?;
    bootstrap::init_runtime()?;

    run(command, &ctx, &mut config)
}
