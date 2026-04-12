use anyhow::Result;

use crate::cli::Command;
use crate::commands::run as dispatch_command_impl;
use crate::{CommandContext, database};

mod config;
mod process;

pub(crate) fn run(command: Command, verbosity: u8) -> Result<()> {
    let mut app_config = config::load()?;
    let context = config::build_context(&app_config, verbosity)?;

    process::init(&context)?;
    dispatch_command(command, &context, &mut app_config)
}

fn dispatch_command(
    command: Command,
    context: &CommandContext,
    config: &mut database::Config,
) -> Result<()> {
    dispatch_command_impl(command, context, config)
}
