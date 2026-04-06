use anyhow::Result;

use crate::AppContext;
use crate::cli::Command;

pub mod command_errors;
pub mod config;
pub mod doctor;
pub mod info;
pub mod install;
pub mod list;
pub mod remove;
pub mod search;
pub mod update;
pub mod version;

pub fn run(command: Command, ctx: &AppContext) -> Result<()> {
    match command {
        Command::List { query } => list::run(ctx, &query),
        Command::Install {
            query,
            ignore_checksum_security,
        } => install::run(ctx, &query, ignore_checksum_security),
        Command::Search { query } => search::run(ctx, &query),
        Command::Info => info::run(ctx),
        Command::Version => version::run(ctx),
        Command::Doctor => doctor::run(ctx),
        Command::Update => update::run(ctx),
        Command::Remove { name, yes, force } => remove::run(ctx, &name, yes, force),
        Command::Config { command } => config::run(ctx, command),
    }
}
