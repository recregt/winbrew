//! Process bootstrap helpers for the CLI.
//!
//! This module owns the side effects that must happen before command execution
//! can safely proceed. The goal is not to implement business logic here, but to
//! make the process contract explicit:
//!
//! - install Ctrl+C handling early so cancellation is consistent across the
//!   whole process;
//! - recover any interrupted installations before a command touches the same
//!   database rows or filesystem paths;
//! - keep the code that mutates process-global state separate from the command
//!   dispatcher so startup remains easy to audit.
//!
//! The public entry point, [`init_runtime`], performs those bootstrap-only
//! actions in the order required by the rest of the CLI. Logging setup lives in
//! [`logging`] because it is a one-time process concern, while stale-install
//! recovery lives in [`cleanup`] because it is a startup-only repair step.

use anyhow::Result;

use crate::cancel;

pub mod cleanup;
pub mod logging;

/// Initialize runtime services that are required before any command runs.
/// The sequence is intentionally minimal but strict. First the Ctrl+C handler
/// is installed so command work can be interrupted safely. Then startup-only
/// cleanup runs so any package rows left in the `Installing` state from a prior
/// crash are marked failed and their leftover filesystem artifacts are removed.
///
/// This function is called after the database pool has been initialized and
/// after logging has already been set up by the caller, so failures can be
/// reported and the cleanup step can query the current install state.
pub fn init_runtime() -> Result<()> {
    cancel::init_handler()?;
    cleanup::cleanup_stale_installations()?;

    Ok(())
}
