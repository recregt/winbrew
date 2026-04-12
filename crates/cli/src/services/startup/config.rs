//! Configuration capture for CLI startup.
//!
//! This submodule converts the persisted storage configuration into the
//! runtime command context. It does not start any process services; that work
//! belongs in [`super::process`]. The split keeps startup easy to reason about:
//! config loading answers "what should this process do?" while process
//! initialization answers "what global state does it need to do it?".

use anyhow::Result;

use crate::{CommandContext, database};

/// Load the current persisted configuration snapshot used by CLI startup.
///
/// This reads the active storage-backed configuration, including any path
/// overrides that are already part of the environment-controlled config model.
pub(super) fn load() -> Result<database::Config> {
    database::Config::load_current()
}

/// Build the command context that will be used for the rest of the process.
///
/// The returned [`CommandContext`] combines application runtime state with UI
/// preferences from the loaded config. The `verbosity` value only affects the
/// runtime application context; it does not rewrite the persisted config.
pub(super) fn build_context(config: &database::Config, verbosity: u8) -> Result<CommandContext> {
    CommandContext::from_config_with_verbosity(config, verbosity)
}
