use anyhow::Result;

use crate::database::Config;
pub use crate::models::domains::shared::{ConfigSection, ConfigValue, ConfigValueSource};

pub fn list_sections(config: &Config) -> Result<Vec<ConfigSection>> {
    config.effective_sections()
}

pub fn get_display_value(config: &Config, key: &str) -> Result<ConfigValue> {
    let (value, source) = config.effective_value(key)?;

    Ok(ConfigValue { value, source })
}

pub fn set_value(config: &mut Config, key: &str, value: &str) -> Result<()> {
    config.set_value(key, value)?;
    config.save_default()?;
    Ok(())
}

pub fn unset_value(config: &mut Config, key: &str) -> Result<()> {
    config.unset_value(key)?;
    config.save_default()?;
    Ok(())
}
