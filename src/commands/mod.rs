use anyhow::Result;

use crate::cli::Command;

pub mod install;
pub mod list;
pub mod remove;

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::List => list::run(),
        Command::Install { name, version } => install::run(&name, &version),
        Command::Remove { name, yes } => remove::run(&name, yes),
    }
}
