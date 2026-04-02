use anyhow::Result;

use crate::database::{self, ConfigSection, ConfigSource};

pub fn list_sections() -> Result<Vec<ConfigSection>> {
    database::config_sections()
}

pub fn get_value(key: &str) -> Result<(String, ConfigSource)> {
    database::get_effective_value(key)
}

pub fn set_value(key: &str, value: &str) -> Result<()> {
    database::config_set(key, value)
}