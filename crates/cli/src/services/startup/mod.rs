//! CLI startup orchestration for `winbrew-cli`.
//!
//! This module owns the pre-dispatch portion of the command lifecycle. The
//! crate entrypoint [`crate::run_app`] delegates here so startup policy stays
//! with the CLI layer instead of leaking into `winbrew-bin`.
//!
//! The workflow is intentionally split into three phases:
//!
//! 1. Load the persisted configuration from storage.
//! 2. Build a [`crate::CommandContext`] from that configuration and the current
//!    verbosity level.
//! 3. Initialize process services and dispatch the selected command.
//!
//! The order is important because:
//!
//! - configuration must be loaded before the effective log and database paths
//!   are known;
//! - logging must start before later startup failures are emitted;
//! - database initialization must happen before runtime cleanup, because the
//!   bootstrap cleanup uses [`crate::database::get_conn`];
//! - command dispatch runs last, after the process is fully hydrated.

use anyhow::Result;

use crate::cli::Command;
use crate::commands::run as dispatch_command_impl;
use crate::{CommandContext, database};

mod config;
mod process;

/// Run the full CLI startup sequence and then execute the selected command.
///
/// The `verbosity` value is forwarded into
/// [`crate::CommandContext::from_config_with_verbosity`] so the process-level
/// runtime state can react to the caller's requested verbosity without mutating
/// the persisted configuration snapshot.
pub(crate) fn run(command: Command, verbosity: u8) -> Result<()> {
    let mut app_config = config::load()?;
    let context = config::build_context(&app_config, verbosity)?;

    process::init(&context)?;
    dispatch_command(command, &context, &mut app_config)
}

/// Hand the hydrated context to the CLI dispatcher after startup side effects
/// have completed.
fn dispatch_command(
    command: Command,
    context: &CommandContext,
    config: &mut database::Config,
) -> Result<()> {
    dispatch_command_impl(command, context, config)
}
