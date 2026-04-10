#![cfg(windows)]

use anyhow::Result;

pub mod cli;
pub mod commands;
pub mod services;

use crate::commands::run;
use crate::services::bootstrap;
use crate::services::shared::config as shared_config;
pub use winbrew_app as app;
pub use winbrew_install::storage as database;
pub use winbrew_install::{cancel, catalog, core, engines, models};
pub use winbrew_ui::{Ui, UiSettings};

// Re-export AppContext from app crate for bin/commands
pub use app::AppContext;

pub fn run_app(command: crate::cli::Command) -> Result<()> {
    let mut config = shared_config::load_current()?;
    let ctx = AppContext::from_config(&config)?;

    bootstrap::logging::init(&ctx.paths.logs, &ctx.log_level, &ctx.file_log_level)?;
    database::init(&ctx.paths)?;
    bootstrap::init_runtime()?;

    run(command, &ctx, &mut config)
}
