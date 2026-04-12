//! Process-global service initialization for CLI startup.
//!
//! This submodule owns the side effects that must happen before any command is
//! dispatched: tracing setup, database initialization, and runtime cleanup.
//! Keeping these steps here makes the startup contract explicit and prevents
//! them from being mixed into config loading or command dispatch.
//!
//! The order is deliberate:
//!
//! - [`crate::services::bootstrap::logging::init`] must run first so later
//!   failures can be written to the console and file sink.
//! - [`crate::database::init`] seeds the process-wide connection state used by
//!   command handlers and cleanup routines.
//! - [`crate::services::bootstrap::init_runtime`] installs Ctrl+C handling and
//!   replays bootstrap cleanup, which depends on the database already being
//!   available.

use anyhow::Result;

use crate::CommandContext;
use crate::database;
use crate::services::bootstrap;

/// Initialize the global services required for a fully hydrated CLI process.
///
/// This is intentionally a side-effecting step. It does not inspect or mutate
/// the command itself; it only prepares process-wide facilities that command
/// execution relies on.
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
