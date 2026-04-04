#[cfg(not(windows))]
compile_error!("winbrew only builds on Windows");

#[cfg(windows)]
use anyhow::Result;

#[cfg(windows)]
use clap::Parser;

pub mod cli;
pub mod commands;
pub mod core;
pub mod database;
pub mod engines;
pub mod models;
pub mod services;
pub mod ui;
pub mod windows;

pub use cli::{Cli, Command};
pub use commands::run;

#[cfg(windows)]
pub fn run_app() -> Result<()> {
    let config = database::Config::current();
    let paths = config.resolved_paths();

    ui::init_settings(ui::UiSettings {
        color_enabled: config.core.color,
        default_yes: config.core.default_yes,
    });

    core::logging::init(&paths, &config.core.log_level, &config.core.file_log_level)?;

    let cli = Cli::parse();

    database::init()?;

    run(cli.command)
}
