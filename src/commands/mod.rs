use anyhow::Result;

use crate::cli::Command;

pub mod config;
pub mod doctor;
pub mod info;
pub mod install;
pub mod list;
pub mod remove;

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::List { query } => list::run(&query),
        Command::Info => info::run(),
        Command::Doctor => doctor::run(),
        Command::Install { query, version } => install::run(&query, version.as_deref()),
        Command::Remove { name, yes, force } => remove::run(&name, yes, force),
        Command::Config { command } => config::run(command),
    }
}
