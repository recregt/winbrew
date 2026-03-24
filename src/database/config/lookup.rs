use anyhow::{Result, anyhow, bail};
use tracing::warn;

use super::keys::{env_override, section_key};
use super::types::{Config, ConfigSection, ConfigSource};

struct SectionSpec {
    title: &'static str,
    entries: Vec<EntrySpec>,
}

struct EntrySpec {
    key: &'static str,
    value: Option<String>,
    display_value: String,
}

impl EntrySpec {
    fn required(key: &'static str, value: String) -> Self {
        Self {
            key,
            display_value: value.clone(),
            value: Some(value),
        }
    }

    fn optional(key: &'static str, value: Option<String>, display_value: String) -> Self {
        Self {
            key,
            value,
            display_value,
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
                    EntrySpec::required("download_timeout", self.core.download_timeout.to_string()),
                    EntrySpec::required(
                        "concurrent_downloads",
                        self.core.concurrent_downloads.to_string(),
                    ),
                    EntrySpec::optional(
                        "proxy",
                        self.core.proxy.clone(),
                        self.core
                            .proxy
                            .clone()
                            .unwrap_or_else(|| "(none)".to_string()),
                    ),
                    EntrySpec::optional(
                        "github_token",
                        self.core.github_token.clone(),
                        self.core
                            .github_token
                            .as_ref()
                            .map(|_| "(set)".to_string())
                            .unwrap_or_else(|| "(unset)".to_string()),
                    ),
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
            SectionSpec {
                title: "Sources",
                entries: vec![
                    EntrySpec::required("primary", self.sources.primary.clone()),
                    EntrySpec::required("winget.url", self.sources.winget.url.clone()),
                    EntrySpec::required("winget.format", self.sources.winget.format.clone()),
                    EntrySpec::required(
                        "winget.manifest_kind",
                        self.sources.winget.manifest_kind.clone(),
                    ),
                    EntrySpec::required(
                        "winget.manifest_path_template",
                        self.sources.winget.manifest_path_template.clone(),
                    ),
                    EntrySpec::required("winget.enabled", self.sources.winget.enabled.to_string()),
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

    pub fn effective_value(&self, key: &str) -> Result<(String, ConfigSource)> {
        self.lookup_effective(key)?.ok_or_else(|| {
            let key = key.trim();
            anyhow!("config key '{key}' not found")
        })
    }

    pub fn effective_optional_value(&self, key: &str) -> Result<Option<(String, ConfigSource)>> {
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

    pub fn get_value(&self, key: &str) -> Result<Option<String>> {
        let key = key.trim();

        if key.is_empty() {
            bail!("config key cannot be empty");
        }

        for section in self.section_specs() {
            for entry in section.entries {
                if section_key(section.title, entry.key) == key {
                    return Ok(entry.value);
                }
            }
        }

        Err(anyhow!("unknown config key: {key}"))
    }

    fn lookup_effective(&self, key: &str) -> Result<Option<(String, ConfigSource)>> {
        let key = key.trim();

        if key.is_empty() {
            bail!("config key cannot be empty");
        }

        if let Some(value) = env_override(key) {
            return Ok(Some((value, ConfigSource::Env)));
        }

        Ok(self
            .get_value(key)?
            .map(|value| (value, ConfigSource::File)))
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap()
    }

    struct TestEnvVar {
        key: &'static str,
    }

    impl TestEnvVar {
        fn set(key: &'static str, value: &str) -> Self {
            // Rust 2024 makes env mutation unsafe because it can race with readers.
            unsafe {
                std::env::set_var(key, value);
            }

            Self { key }
        }
    }

    impl Drop for TestEnvVar {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn get_value_returns_none_for_unset_optional_fields() {
        let _guard = env_lock();
        let config = Config::default();

        assert_eq!(config.get_value("core.proxy").unwrap(), None);
        assert_eq!(config.get_value("core.github_token").unwrap(), None);
    }

    #[test]
    fn effective_optional_value_returns_none_for_unset_optional_fields() {
        let _guard = env_lock();
        let config = Config::default();

        assert_eq!(config.effective_optional_value("core.proxy").unwrap(), None);
        assert_eq!(
            config
                .effective_optional_value("core.github_token")
                .unwrap(),
            None
        );
    }

    #[test]
    fn effective_optional_value_prefers_env_override() {
        let _guard = env_lock();
        let _env = TestEnvVar::set("WINBREW_CORE_PROXY", "http://localhost:8080");
        let config = Config::default();

        assert_eq!(
            config.effective_optional_value("core.proxy").unwrap(),
            Some((
                "http://localhost:8080".to_string(),
                super::ConfigSource::Env,
            ))
        );
    }
}
