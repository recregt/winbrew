use anyhow::Result;

use crate::AppContext;
use crate::database;
pub use crate::models::config::{ConfigSection, ConfigValue, ConfigValueSource};

pub fn list_sections(ctx: &AppContext) -> Vec<ConfigSection> {
    ctx.sections.clone()
}

pub fn get_display_value(key: &str) -> Result<ConfigValue> {
    let (value, source) = database::get_effective_value(key)?;

    Ok(ConfigValue {
        value,
        source: source.into(),
    })
}

pub fn set_value(key: &str, value: &str) -> Result<()> {
    database::config_set(key, value)
}
