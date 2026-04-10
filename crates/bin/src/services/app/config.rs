use anyhow::Result;

use crate::AppContext;
pub use crate::models::{ConfigSection, ConfigValue, ConfigValueSource};
use crate::services::shared::storage;

pub fn list_sections(ctx: &AppContext) -> Vec<ConfigSection> {
    ctx.sections.clone()
}

pub fn get_display_value(key: &str) -> Result<ConfigValue> {
    let (value, source) = storage::get_effective_value(key)?;

    Ok(ConfigValue { value, source })
}

pub fn set_value(key: &str, value: &str) -> Result<()> {
    storage::config_set(key, value)
}

pub fn unset_value(key: &str) -> Result<()> {
    storage::config_unset(key)
}
