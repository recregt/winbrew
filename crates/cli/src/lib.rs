#![cfg(windows)]

use anyhow::Result;
use std::io;

pub mod cli;
pub mod commands;
pub mod services;

use winbrew_app::AppContext;
use winbrew_ui::{Ui, UiSettings};

use crate::commands::run;
use crate::services::bootstrap;

pub(crate) use winbrew_app as app;
pub use winbrew_app::core::cancel;
pub use winbrew_app::{core, engines, models, storage as database};

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

    pub fn ui(&self) -> Ui<io::Stdout> {
        Ui::new(self.ui)
    }

    pub fn app(&self) -> &AppContext {
        &self.app
    }
}

pub fn run_app(command: crate::cli::Command, verbosity: u8) -> Result<()> {
    let mut config = database::Config::load_current()?;
    let ctx = CommandContext::from_config_with_verbosity(&config, verbosity)?;

    bootstrap::logging::init(
        &ctx.app().paths.logs,
        &ctx.app().log_level,
        &ctx.app().file_log_level,
    )?;
    database::init(&ctx.app().paths)?;
    bootstrap::init_runtime()?;

    run(command, &ctx, &mut config)
}
