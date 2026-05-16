use anyhow::Result;
use std::borrow::Cow;

use super::error::{ConfigError, ConfigResult};
use super::types::{Config, ConfigSection, ConfigSource};

struct SectionSpec {
    title: &'static str,
    entries: &'static [EntrySpec],
}

struct EntrySpec {
    key: &'static str,
    full_key: &'static str,
    value: for<'a> fn(&'a Config) -> Cow<'a, str>,
}

const SECTION_SPECS: &[SectionSpec] = &[
    SectionSpec {
        title: "Core",
        entries: &[
            EntrySpec {
                key: "log_level",
                full_key: "core.log_level",
                value: core_log_level,
            },
            EntrySpec {
                key: "file_log_level",
                full_key: "core.file_log_level",
                value: core_file_log_level,
            },
            EntrySpec {
                key: "auto_update",
                full_key: "core.auto_update",
                value: core_auto_update,
            },
            EntrySpec {
                key: "confirm_remove",
                full_key: "core.confirm_remove",
                value: core_confirm_remove,
            },
            EntrySpec {
                key: "default_yes",
                full_key: "core.default_yes",
                value: core_default_yes,
            },
            EntrySpec {
                key: "color",
                full_key: "core.color",
                value: core_color,
            },
        ],
    },
    SectionSpec {
        title: "Paths",
        entries: &[
            EntrySpec {
                key: "root",
                full_key: "paths.root",
                value: paths_root,
            },
            EntrySpec {
                key: "packages",
                full_key: "paths.packages",
                value: paths_packages,
            },
            EntrySpec {
                key: "data",
                full_key: "paths.data",
                value: paths_data,
            },
            EntrySpec {
                key: "logs",
                full_key: "paths.logs",
                value: paths_logs,
            },
            EntrySpec {
                key: "cache",
                full_key: "paths.cache",
                value: paths_cache,
            },
        ],
    },
];

fn core_log_level(config: &Config) -> Cow<'_, str> {
    Cow::Borrowed(config.core.log_level.as_str())
}

fn core_file_log_level(config: &Config) -> Cow<'_, str> {
    Cow::Borrowed(config.core.file_log_level.as_str())
}

fn core_auto_update(config: &Config) -> Cow<'_, str> {
    Cow::Owned(config.core.auto_update.to_string())
}

fn core_confirm_remove(config: &Config) -> Cow<'_, str> {
    Cow::Owned(config.core.confirm_remove.to_string())
}

fn core_default_yes(config: &Config) -> Cow<'_, str> {
    Cow::Owned(config.core.default_yes.to_string())
}

fn core_color(config: &Config) -> Cow<'_, str> {
    Cow::Owned(config.core.color.to_string())
}

fn paths_root(config: &Config) -> Cow<'_, str> {
    Cow::Borrowed(config.paths.root.as_str())
}

fn paths_packages(config: &Config) -> Cow<'_, str> {
    Cow::Borrowed(config.paths.packages.as_str())
}

fn paths_data(config: &Config) -> Cow<'_, str> {
    Cow::Borrowed(config.paths.data.as_str())
}

fn paths_logs(config: &Config) -> Cow<'_, str> {
    Cow::Borrowed(config.paths.logs.as_str())
}

fn paths_cache(config: &Config) -> Cow<'_, str> {
    Cow::Borrowed(config.paths.cache.as_str())
}

fn find_entry(key: &str) -> Option<&'static EntrySpec> {
    SECTION_SPECS
        .iter()
        .flat_map(|section| section.entries.iter())
        .find(|entry| entry.full_key == key)
}

impl Config {
    pub fn sections(&self) -> Vec<ConfigSection> {
        SECTION_SPECS
            .iter()
            .map(|section| ConfigSection {
                title: section.title.to_string(),
                entries: section
                    .entries
                    .iter()
                    .map(|entry| (entry.key.to_string(), (entry.value)(self).into_owned()))
                    .collect(),
            })
            .collect()
    }

    pub fn effective_value(&self, key: &str) -> ConfigResult<(String, ConfigSource)> {
        self.lookup_effective(key)?
            .ok_or_else(|| ConfigError::UnknownKey {
                key: key.trim().to_string(),
            })
    }

    pub fn effective_optional_value(
        &self,
        key: &str,
    ) -> ConfigResult<Option<(String, ConfigSource)>> {
        self.lookup_effective(key)
    }

    pub fn effective_sections(&self) -> Result<Vec<ConfigSection>> {
        let mut sections = Vec::new();

        for section in SECTION_SPECS {
            let mut entries = Vec::with_capacity(section.entries.len());

            for entry in section.entries {
                let display_value = if let Some(value) = self.env.value(entry.full_key) {
                    format!("{value} [env override]")
                } else {
                    (entry.value)(self).into_owned()
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

        Ok(find_entry(key).map(|entry| (entry.value)(self).into_owned()))
    }

    fn lookup_effective(&self, key: &str) -> ConfigResult<Option<(String, ConfigSource)>> {
        let key = key.trim();

        if key.is_empty() {
            return Err(ConfigError::EmptyKey);
        }

        let Some(entry) = find_entry(key) else {
            return Ok(None);
        };

        if let Some(value) = self.env.value(key) {
            return Ok(Some((value.to_string(), ConfigSource::Env)));
        }

        Ok(Some(((entry.value)(self).into_owned(), ConfigSource::File)))
    }
}
