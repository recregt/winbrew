use anyhow::Result;

use crate::{CommandContext, database};

pub(super) fn load() -> Result<database::Config> {
    database::Config::load_current()
}

pub(super) fn build_context(config: &database::Config, verbosity: u8) -> Result<CommandContext> {
    CommandContext::from_config_with_verbosity(config, verbosity)
}
