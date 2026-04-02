use anyhow::Result;
use tracing::warn;

use super::errors::{ConfigError, ConfigResult};
use super::keys::section_key;
use super::types::{Config, ConfigSection, ConfigSource};

struct SectionSpec {
    title: &'static str,
    entries: Vec<EntrySpec>,
}

struct EntrySpec {
    key: &'static str,
    value: String,
    display_value: String,
}

impl EntrySpec {
    fn required(key: &'static str, value: String) -> Self {
        Self {
            key,
            display_value: value.clone(),
            value,
        }
    }
}

impl Config {
    fn section_specs(&self) -> Vec<SectionSpec> {
        vec![
            SectionSpec {
                title: "Core",
                entries: vec![
                    EntrySpec::required("log_level", self.core.log_level.clone()),
                    EntrySpec::required("file_log_level", self.core.file_log_level.clone()),
                    EntrySpec::required("auto_update", self.core.auto_update.to_string()),
                    EntrySpec::required("confirm_remove", self.core.confirm_remove.to_string()),
                    EntrySpec::required("default_yes", self.core.default_yes.to_string()),
                    EntrySpec::required("color", self.core.color.to_string()),
                ],
            },
            SectionSpec {
                title: "Paths",
                entries: vec![
                    EntrySpec::required("root", self.paths.root.clone()),
                    EntrySpec::required("packages", self.paths.packages.clone()),
                    EntrySpec::required("data", self.paths.data.clone()),
                    EntrySpec::required("logs", self.paths.logs.clone()),
                    EntrySpec::required("cache", self.paths.cache.clone()),
                ],
            },
        ]
    }

    pub fn sections(&self) -> Vec<ConfigSection> {
        self.section_specs()
            .into_iter()
            .map(|section| ConfigSection {
                title: section.title.to_string(),
                entries: section
                    .entries
                    .into_iter()
                    .map(|entry| (entry.key.to_string(), entry.display_value))
                    .collect(),
            })
            .collect()
    }

    pub fn effective_value(&self, key: &str) -> ConfigResult<(String, ConfigSource)> {
        self.lookup_effective(key)?.ok_or_else(|| ConfigError::UnknownKey {
            key: key.trim().to_string(),
        })
    }

    pub fn effective_optional_value(&self, key: &str) -> ConfigResult<Option<(String, ConfigSource)>> {
        self.lookup_effective(key)
    }

    pub fn effective_sections(&self) -> Result<Vec<ConfigSection>> {
        let mut sections = Vec::new();

        for section in self.section_specs() {
            let mut entries = Vec::with_capacity(section.entries.len());

            for entry in section.entries {
                let full_key = section_key(section.title, entry.key);

                let display_value = match self.lookup_effective(&full_key) {
                    Ok(Some((value, ConfigSource::Env))) => {
                        format!("{value} [env override]")
                    }
                    Ok(Some((value, ConfigSource::File))) => value,
                    Ok(None) => entry.display_value,
                    Err(err) => {
                        warn!(key = %full_key, error = %err, "falling back to file config value");
                        entry.display_value
                    }
                };

                entries.push((entry.key.to_string(), display_value));
            }

            sections.push(ConfigSection {
                title: section.title.to_string(),
                entries,
            });
        }

        Ok(sections)
    }

    pub fn get_value(&self, key: &str) -> ConfigResult<Option<String>> {
        let key = key.trim();

        if key.is_empty() {
            return Err(ConfigError::EmptyKey);
        }

        for section in self.section_specs() {
            for entry in section.entries {
                if section_key(section.title, entry.key) == key {
                    return Ok(Some(entry.value.clone()));
                }
            }
        }

        Err(ConfigError::UnknownKey { key: key.to_string() })
    }

    fn lookup_effective(&self, key: &str) -> ConfigResult<Option<(String, ConfigSource)>> {
        let key = key.trim();

        if key.is_empty() {
            return Err(ConfigError::EmptyKey);
        }

        if let Some(value) = self.env.value(key) {
            return Ok(Some((value.to_string(), ConfigSource::Env)));
        }

        Ok(self
            .get_value(key)?
            .map(|value| (value, ConfigSource::File)))
    }
}
