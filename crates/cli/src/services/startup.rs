use anyhow::Result;

use crate::cli::Command;
use crate::commands::run as dispatch_command_impl;
use crate::services::bootstrap;
use crate::{CommandContext, database};

pub(crate) fn run(command: Command, verbosity: u8) -> Result<()> {
    let mut config = load_config()?;
    let context = build_context(&config, verbosity)?;

    init_process_services(&context)?;
    dispatch_command(command, &context, &mut config)
}

fn load_config() -> Result<database::Config> {
    database::Config::load_current()
}

fn build_context(config: &database::Config, verbosity: u8) -> Result<CommandContext> {
    CommandContext::from_config_with_verbosity(config, verbosity)
}

fn init_process_services(context: &CommandContext) -> Result<()> {
    bootstrap::logging::init(
        &context.app().paths.logs,
        &context.app().log_level,
        &context.app().file_log_level,
    )?;
    database::init(&context.app().paths)?;
    bootstrap::init_runtime()?;

    Ok(())
}

fn dispatch_command(
    command: Command,
    context: &CommandContext,
    config: &mut database::Config,
) -> Result<()> {
    dispatch_command_impl(command, context, config)
}
