mod cli;
mod commands;
mod core;
mod database;
mod manifest;
mod operations;
mod ui;
mod windows;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let conn = database::connect()?;
    database::migrate(&conn)?;
    let cli = cli::Cli::parse();
    commands::run(cli.command)
}
