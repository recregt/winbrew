#![cfg(windows)]

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;

use crate::cli::Cli;
use crate::commands::run;
use crate::core::paths::ResolvedPaths;
use crate::services::config::ConfigSection;

pub mod cli;
pub mod commands;
pub mod core;
pub mod database;
pub mod engines;
pub mod models;
pub mod services;
pub mod ui;
pub mod windows;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub ui: ui::UiSettings,
    pub paths: ResolvedPaths,
    pub sections: Vec<ConfigSection>,
    pub root_from_env: bool,
    pub log_level: Arc<str>,
    pub file_log_level: Arc<str>,
}

impl AppContext {
    pub fn from_config(config: crate::database::Config) -> Result<Self> {
        let paths = config.resolved_paths();
        let sections = config
            .effective_sections()?
            .into_iter()
            .map(Into::into)
            .collect();

        Ok(Self {
            ui: ui::UiSettings {
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
    let config = database::Config::load_current()?;
    let ctx = AppContext::from_config(config)?;

    core::logging::init(&ctx)?;
    database::init(&ctx.paths)?;

    let cli = Cli::parse();
    run(cli.command, &ctx)
}
