use anyhow::Result;

use crate::AppContext;
use crate::database;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSection {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    pub value: String,
    pub source: ConfigValueSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValueSource {
    Env,
    File,
}

impl From<database::ConfigSection> for ConfigSection {
    fn from(section: database::ConfigSection) -> Self {
        Self {
            title: section.title,
            entries: section.entries,
        }
    }
}

impl From<database::ConfigSource> for ConfigValueSource {
    fn from(source: database::ConfigSource) -> Self {
        match source {
            database::ConfigSource::Env => ConfigValueSource::Env,
            database::ConfigSource::File => ConfigValueSource::File,
        }
    }
}

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
