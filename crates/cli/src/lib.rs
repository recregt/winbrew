#![cfg(windows)]

use anyhow::Result;

pub mod cli;
pub mod commands;
pub mod services;

use crate::commands::run;
use crate::services::bootstrap;

pub use winbrew_app as app;
pub use winbrew_app::core::cancel;
pub use winbrew_app::{core, engines, models, storage as database};
pub use winbrew_ui::{Ui, UiSettings};

pub use app::AppContext;

pub fn run_app(command: crate::cli::Command, verbosity: u8) -> Result<()> {
    let mut config = database::Config::load_current()?;
    let ctx = AppContext::from_config_with_verbosity(&config, verbosity)?;

    bootstrap::logging::init(&ctx.paths.logs, &ctx.log_level, &ctx.file_log_level)?;
    database::init(&ctx.paths)?;
    bootstrap::init_runtime()?;

    run(command, &ctx, &mut config)
}
