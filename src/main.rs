mod core;
mod cli;
mod commands;
mod operations;
mod ui;
mod windows;
mod database;
mod manifest;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let conn = database::connect()?;
    database::migrate(&conn)?;
    let cli = cli::Cli::parse();
    commands::run(cli.command)
}
