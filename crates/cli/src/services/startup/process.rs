use anyhow::Result;

use crate::CommandContext;
use crate::database;
use crate::services::bootstrap;

pub(super) fn init(context: &CommandContext) -> Result<()> {
    bootstrap::logging::init(
        &context.app().paths.logs,
        &context.app().log_level,
        &context.app().file_log_level,
    )?;
    database::init(&context.app().paths)?;
    bootstrap::init_runtime()?;

    Ok(())
}
