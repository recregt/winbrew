use std::path::Path;

use super::types::{Config, ConfigSection};
use crate::core::fs::atomic_write_with_pid_suffix;
use anyhow::Result;

pub(crate) fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    atomic_write_with_pid_suffix(path, contents)
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load_default()?;

    config.set_value(key, value)?;
    config.save_default()?;
    Ok(())
}

pub fn config_sections() -> Result<Vec<ConfigSection>> {
    Config::load_current()?.effective_sections()
}

pub fn get_effective_value(key: &str) -> Result<(String, super::types::ConfigSource)> {
    Ok(Config::load_current()?.effective_value(key)?)
}
