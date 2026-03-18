mod cli;
mod commands;
mod core;
mod database;
mod manifest;
mod models;
mod operations;
mod ui;
mod windows;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    commands::run(cli.command)
}
