#![cfg(windows)]

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;

pub mod cli;
pub mod commands;
pub mod services;

pub use winbrew_catalog as catalog;
pub use winbrew_storage as database;

use crate::cli::Cli;
use crate::commands::run;
use crate::services::bootstrap;
use crate::services::shared::config as shared_config;
use winbrew_core as core;
use winbrew_core::paths::ResolvedPaths;
use winbrew_engines as engines;
use winbrew_models::ConfigSection;
use winbrew_ui::{Ui, UiSettings};

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
    pub fn from_config(config: crate::database::Config) -> Result<Self> {
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

pub fn run_app() -> Result<()> {
    let config = shared_config::load_current()?;
    let ctx = AppContext::from_config(config)?;

    bootstrap::logging::init(&ctx.paths.logs, &ctx.log_level, &ctx.file_log_level)?;
    database::init(&ctx.paths)?;
    bootstrap::init_runtime()?;

    let cli = Cli::parse();
    run(cli.command, &ctx)
}
