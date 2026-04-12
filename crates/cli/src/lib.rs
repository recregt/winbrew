#![cfg(windows)]

use anyhow::Result;
use std::ops::{Deref, DerefMut};

pub mod cli;
pub mod commands;
pub mod services;

use winbrew_app::AppContext;

use crate::commands::run;
use crate::services::bootstrap;

pub(crate) use winbrew_app as app;
pub use winbrew_app::core::cancel;
pub use winbrew_app::{core, engines, models, storage as database};
pub use winbrew_ui::{Ui, UiSettings};

#[derive(Debug, Clone)]
pub struct CommandContext {
    app: AppContext,
    ui: UiSettings,
}

impl CommandContext {
    pub fn from_config(config: &database::Config) -> Result<Self> {
        Self::from_config_with_verbosity(config, 0)
    }

    pub fn from_config_with_verbosity(config: &database::Config, verbosity: u8) -> Result<Self> {
        Ok(Self {
            app: AppContext::from_config_with_verbosity(config, verbosity)?,
            ui: UiSettings {
                color_enabled: config.core.color,
                default_yes: config.core.default_yes,
            },
        })
    }

    pub fn ui_settings(&self) -> UiSettings {
        self.ui
    }
}

impl Deref for CommandContext {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        &self.app
    }
}

impl DerefMut for CommandContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.app
    }
}

pub fn run_app(command: crate::cli::Command, verbosity: u8) -> Result<()> {
    let mut config = database::Config::load_current()?;
    let ctx = CommandContext::from_config_with_verbosity(&config, verbosity)?;

    bootstrap::logging::init(&ctx.paths.logs, &ctx.log_level, &ctx.file_log_level)?;
    database::init(&ctx.paths)?;
    bootstrap::init_runtime()?;

    run(command, &ctx, &mut config)
}
