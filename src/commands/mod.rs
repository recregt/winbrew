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
        Command::List => list::run(),
        Command::Info => info::run(),
        Command::Doctor => doctor::run(),
        Command::Install { name, version } => install::run(&name, &version),
        Command::Remove { name, yes } => remove::run(&name, yes),
        Command::Config { command } => config::run(command),
    }
}
