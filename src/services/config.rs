use anyhow::Result;

use crate::database::{self, ConfigSection, ConfigSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    pub value: String,
    pub overridden_by_env: bool,
}

pub fn list_sections() -> Result<Vec<ConfigSection>> {
    database::config_sections()
}

pub fn get_value(key: &str) -> Result<(String, ConfigSource)> {
    database::get_effective_value(key)
}

pub fn get_display_value(key: &str) -> Result<ConfigValue> {
    let (value, source) = get_value(key)?;

    Ok(ConfigValue {
        value,
        overridden_by_env: matches!(source, ConfigSource::Env),
    })
}

pub fn set_value(key: &str, value: &str) -> Result<()> {
    database::config_set(key, value)
}
