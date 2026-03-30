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
pub mod models;
pub mod services;
pub mod ui;
pub mod windows;

pub use cli::{Cli, Command};
pub use commands::run;

#[cfg(windows)]
pub fn run_app() -> Result<()> {
    core::logging::init()?;

    let cli = Cli::parse();

    database::init()?;

    run(cli.command)
}
