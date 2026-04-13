#![cfg(windows)]

//! Command-line facade for WinBrew.
//!
//! `winbrew-cli` owns command parsing, command dispatch, and the terminal UI
//! context used by the binary. It keeps the executable thin while translating
//! parsed commands into app-layer operations.
//!
//! Public modules:
//!
//! - `cli`: Clap command definitions and argument parsing
//! - `commands`: wrapper handlers and command-specific UI behavior
//! - `services`: startup and bootstrap wiring used by the binary

use anyhow::Result;
use std::io;

pub mod cli;
pub mod commands;
pub mod services;

use winbrew_app::AppContext;
use winbrew_ui::{Ui, UiSettings};

pub(crate) use winbrew_app as app;
pub use winbrew_app::core::cancel;
pub use winbrew_app::{core, engines, models, storage as database};

#[derive(Debug, Clone)]
pub struct CommandContext {
    app: AppContext,
    ui: UiSettings,
}

impl CommandContext {
    /// Build a command context from a loaded configuration.
    pub fn from_config(config: &database::Config) -> Result<Self> {
        Self::from_config_with_verbosity(config, 0)
    }

    /// Build a command context with an explicit verbosity level.
    pub fn from_config_with_verbosity(config: &database::Config, verbosity: u8) -> Result<Self> {
        Ok(Self {
            app: AppContext::from_config_with_verbosity(config, verbosity)?,
            ui: UiSettings {
                color_enabled: config.core.color,
                default_yes: config.core.default_yes,
            },
        })
    }

    /// Create a terminal UI for the current command invocation.
    pub fn ui(&self) -> Ui<io::Stdout> {
        Ui::new(self.ui)
    }

    /// Return the application context used by the command handlers.
    pub fn app(&self) -> &AppContext {
        &self.app
    }
}

/// Run a parsed command through the bootstrap pipeline.
pub fn run_app(command: crate::cli::Command, verbosity: u8) -> Result<()> {
    services::startup::run(command, verbosity)
}
