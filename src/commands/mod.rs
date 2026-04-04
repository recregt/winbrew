use anyhow::Result;

use crate::cli::Command;

pub mod config;
pub mod doctor;
pub mod info;
pub mod install;
pub mod list;
pub mod remove;
pub mod search;
pub mod update;
pub mod version;

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::List { query } => list::run(&query),
        Command::Install { query } => install::run(&query),
        Command::Search { query } => search::run(&query),
        Command::Info => info::run(),
        Command::Version => version::run(),
        Command::Doctor => doctor::run(),
        Command::Update => update::run(),
        Command::Remove { name, yes, force } => remove::run(&name, yes, force),
        Command::Config { command } => config::run(command),
    }
}
