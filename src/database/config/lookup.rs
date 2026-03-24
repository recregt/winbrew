use anyhow::{Result, anyhow, bail};

use super::keys::{env_override, section_key};
use super::types::{Config, ConfigSection};

impl Config {
    pub fn sections(&self) -> Vec<ConfigSection> {
        vec![
            ConfigSection {
                title: "Core".to_string(),
                entries: vec![
                    ("log_level".to_string(), self.core.log_level.clone()),
                    (
                        "file_log_level".to_string(),
                        self.core.file_log_level.clone(),
                    ),
                    ("auto_update".to_string(), self.core.auto_update.to_string()),
                    (
                        "confirm_remove".to_string(),
                        self.core.confirm_remove.to_string(),
                    ),
                    ("default_yes".to_string(), self.core.default_yes.to_string()),
                    ("color".to_string(), self.core.color.to_string()),
                    (
                        "download_timeout".to_string(),
                        self.core.download_timeout.to_string(),
                    ),
                    (
                        "concurrent_downloads".to_string(),
                        self.core.concurrent_downloads.to_string(),
                    ),
                    (
                        "proxy".to_string(),
                        self.core
                            .proxy
                            .clone()
                            .unwrap_or_else(|| "(none)".to_string()),
                    ),
                    (
                        "github_token".to_string(),
                        self.core
                            .github_token
                            .as_ref()
                            .map(|_| "(set)".to_string())
                            .unwrap_or_else(|| "(unset)".to_string()),
                    ),
                ],
            },
            ConfigSection {
                title: "Paths".to_string(),
                entries: vec![
                    ("root".to_string(), self.paths.root.clone()),
                    ("packages".to_string(), self.paths.packages.clone()),
                    ("data".to_string(), self.paths.data.clone()),
                    ("logs".to_string(), self.paths.logs.clone()),
                    ("cache".to_string(), self.paths.cache.clone()),
                ],
            },
            ConfigSection {
                title: "Sources".to_string(),
                entries: vec![
                    ("primary".to_string(), self.sources.primary.clone()),
                    ("winget.url".to_string(), self.sources.winget.url.clone()),
                    (
                        "winget.format".to_string(),
                        self.sources.winget.format.clone(),
                    ),
                    (
                        "winget.manifest_kind".to_string(),
                        self.sources.winget.manifest_kind.clone(),
                    ),
                    (
                        "winget.manifest_path_template".to_string(),
                        self.sources.winget.manifest_path_template.clone(),
                    ),
                    (
                        "winget.enabled".to_string(),
                        self.sources.winget.enabled.to_string(),
                    ),
                ],
            },
        ]
    }

    pub fn effective_value(&self, key: &str) -> Result<(String, &'static str)> {
        let key = key.trim();

        if key.is_empty() {
            bail!("config key cannot be empty");
        }

        if let Some(value) = env_override(key) {
            return Ok((value, "env"));
        }

        let value = self
            .get_value(key)?
            .ok_or_else(|| anyhow!("config key '{key}' not found"))?;

        Ok((value, "file"))
    }

    pub fn effective_optional_value(&self, key: &str) -> Result<Option<(String, &'static str)>> {
        let key = key.trim();

        if key.is_empty() {
            bail!("config key cannot be empty");
        }

        if let Some(value) = env_override(key) {
            return Ok(Some((value, "env")));
        }

        Ok(self.get_value(key)?.map(|value| (value, "file")))
    }

    pub fn effective_sections(&self) -> Result<Vec<ConfigSection>> {
        let mut sections = Vec::new();

        for section in self.sections() {
            let mut entries = Vec::with_capacity(section.entries.len());

            for (key, file_value) in section.entries {
                let full_key = section_key(&section.title, &key);
                let (value, source) = self
                    .effective_value(&full_key)
                    .unwrap_or((file_value, "file"));
                let display_value = if source == "env" {
                    format!("{value} [env override]")
                } else {
                    value
                };

                entries.push((key, display_value));
            }

            sections.push(ConfigSection {
                title: section.title,
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

        Ok(match key {
            "core.log_level" => Some(self.core.log_level.clone()),
            "core.file_log_level" => Some(self.core.file_log_level.clone()),
            "core.auto_update" => Some(self.core.auto_update.to_string()),
            "core.confirm_remove" => Some(self.core.confirm_remove.to_string()),
            "core.default_yes" => Some(self.core.default_yes.to_string()),
            "core.color" => Some(self.core.color.to_string()),
            "core.download_timeout" => Some(self.core.download_timeout.to_string()),
            "core.concurrent_downloads" => Some(self.core.concurrent_downloads.to_string()),
            "core.proxy" => self.core.proxy.clone(),
            "core.github_token" => self.core.github_token.clone(),
            "paths.root" => Some(self.paths.root.clone()),
            "paths.packages" => Some(self.paths.packages.clone()),
            "paths.data" => Some(self.paths.data.clone()),
            "paths.logs" => Some(self.paths.logs.clone()),
            "paths.cache" => Some(self.paths.cache.clone()),
            "sources.primary" => Some(self.sources.primary.clone()),
            "sources.winget.url" => Some(self.sources.winget.url.clone()),
            "sources.winget.format" => Some(self.sources.winget.format.clone()),
            "sources.winget.manifest_kind" => Some(self.sources.winget.manifest_kind.clone()),
            "sources.winget.manifest_path_template" => {
                Some(self.sources.winget.manifest_path_template.clone())
            }
            "sources.winget.enabled" => Some(self.sources.winget.enabled.to_string()),
            _ => return Err(anyhow!("unknown config key: {key}")),
        })
    }
}
