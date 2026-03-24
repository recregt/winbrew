use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::core::paths;

static CONFIG_CACHE: OnceLock<Mutex<Option<Config>>> = OnceLock::new();

const DEFAULT_REGISTRY_URL: &str = "https://raw.githubusercontent.com/microsoft/winget-pkgs/master";
const DEFAULT_WINGET_PATH_TEMPLATE: &str =
    "manifests/${partition}/${publisher}/${package}/${version}/${identifier}.${kind}.yaml";
const DEFAULT_ROOT: &str = r"C:\winbrew";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub core: CoreConfig,

    #[serde(default)]
    pub paths: PathsConfig,

    #[serde(default)]
    pub sources: SourcesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_true")]
    pub auto_update: bool,

    #[serde(default = "default_true")]
    pub confirm_remove: bool,

    #[serde(default)]
    pub default_yes: bool,

    #[serde(default = "default_true")]
    pub color: bool,

    #[serde(default = "default_download_timeout")]
    pub download_timeout: u64,

    #[serde(default = "default_concurrent_downloads")]
    pub concurrent_downloads: u64,

    #[serde(default)]
    pub github_token: Option<String>,

    #[serde(default)]
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_root_path")]
    pub root: String,

    #[serde(default = "default_packages_path")]
    pub packages: String,

    #[serde(default = "default_data_path")]
    pub data: String,

    #[serde(default = "default_logs_path")]
    pub logs: String,

    #[serde(default = "default_cache_path")]
    pub cache: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesConfig {
    #[serde(default = "default_primary_source")]
    pub primary: String,

    #[serde(default)]
    pub winget: SourceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    #[serde(default = "default_registry_url")]
    pub url: String,

    #[serde(default = "default_source_format")]
    pub format: String,

    #[serde(default = "default_manifest_kind")]
    pub manifest_kind: String,

    #[serde(default = "default_manifest_path_template")]
    pub manifest_path_template: String,

    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSection {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            auto_update: default_true(),
            confirm_remove: default_true(),
            default_yes: false,
            color: default_true(),
            download_timeout: default_download_timeout(),
            concurrent_downloads: default_concurrent_downloads(),
            github_token: None,
            proxy: None,
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            root: default_root_path(),
            packages: default_packages_path(),
            data: default_data_path(),
            logs: default_logs_path(),
            cache: default_cache_path(),
        }
    }
}

impl Default for SourcesConfig {
    fn default() -> Self {
        Self {
            primary: default_primary_source(),
            winget: SourceConfig::default(),
        }
    }
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            url: default_registry_url(),
            format: default_source_format(),
            manifest_kind: default_manifest_kind(),
            manifest_path_template: default_manifest_path_template(),
            enabled: default_true(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_download_timeout() -> u64 {
    30
}

fn default_concurrent_downloads() -> u64 {
    3
}

fn default_root_path() -> String {
    DEFAULT_ROOT.to_string()
}

fn default_packages_path() -> String {
    "${root}\\packages".to_string()
}

fn default_data_path() -> String {
    "${root}\\data".to_string()
}

fn default_logs_path() -> String {
    "${root}\\data\\logs".to_string()
}

fn default_cache_path() -> String {
    "${root}\\data\\cache".to_string()
}

fn default_primary_source() -> String {
    "winget".to_string()
}

fn default_registry_url() -> String {
    DEFAULT_REGISTRY_URL.to_string()
}

fn default_source_format() -> String {
    "yaml".to_string()
}

fn default_manifest_kind() -> String {
    "installer".to_string()
}

fn default_manifest_path_template() -> String {
    DEFAULT_WINGET_PATH_TEMPLATE.to_string()
}

fn config_path() -> PathBuf {
    paths::config_file()
}

fn cache() -> &'static Mutex<Option<Config>> {
    CONFIG_CACHE.get_or_init(|| Mutex::new(None))
}

fn lock_cache() -> Result<std::sync::MutexGuard<'static, Option<Config>>> {
    cache()
        .lock()
        .map_err(|_| anyhow!("config cache lock poisoned"))
}

fn update_cache(config: Config) {
    if let Ok(mut guard) = cache().lock() {
        *guard = Some(config);
    }
}

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }

    let temp_path = path.with_extension("toml.tmp");
    {
        let mut file = fs::File::create(&temp_path).context("failed to create temp config file")?;
        file.write_all(contents.as_bytes())
            .context("failed to write temp config file")?;
        file.flush().context("failed to flush temp config file")?;
    }

    if path.exists() {
        fs::remove_file(path).context("failed to replace config file")?;
    }

    fs::rename(&temp_path, path).context("failed to finalize config file")?;
    Ok(())
}

fn parse_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        value => Err(anyhow!("invalid {key} value: {value}")),
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    Ok(Self::default())
                } else {
                    toml::from_str(&contents).context("failed to parse config file")
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(anyhow::Error::new(err).context("failed to read config file")),
        }
    }

    pub fn load_default() -> Result<Self> {
        Self::load(&config_path())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = toml::to_string_pretty(self).context("failed to serialize config file")?;
        atomic_write(path, &contents)
    }

    pub fn save_default(&self) -> Result<()> {
        self.save(&config_path())
    }

    pub fn current() -> Self {
        Self::load_cached().unwrap_or_default()
    }

    pub fn resolved_paths(&self) -> paths::ResolvedPaths {
        let root = PathBuf::from(&self.paths.root);
        paths::resolved_paths(
            &root,
            &self.paths.packages,
            &self.paths.data,
            &self.paths.logs,
            &self.paths.cache,
        )
    }

    pub fn sections(&self) -> Vec<ConfigSection> {
        vec![
            ConfigSection {
                title: "Core".to_string(),
                entries: vec![
                    ("log_level".to_string(), self.core.log_level.clone()),
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

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let key = key.trim();

        if key.is_empty() {
            bail!("config key cannot be empty");
        }

        let value = value.trim();
        validate_config_value(key, value)?;
        let value = normalize_config_value(key, value);

        match key {
            "core.log_level" => self.core.log_level = value.to_string(),
            "core.auto_update" => self.core.auto_update = parse_bool(key, &value)?,
            "core.confirm_remove" => self.core.confirm_remove = parse_bool(key, &value)?,
            "core.default_yes" => self.core.default_yes = parse_bool(key, &value)?,
            "core.color" => self.core.color = parse_bool(key, &value)?,
            "core.download_timeout" => {
                self.core.download_timeout = value
                    .parse::<u64>()
                    .with_context(|| format!("invalid {key} value"))?
            }
            "core.concurrent_downloads" => {
                self.core.concurrent_downloads = value
                    .parse::<u64>()
                    .with_context(|| format!("invalid {key} value"))?
            }
            "core.proxy" => self.core.proxy = parse_value(&value),
            "core.github_token" => self.core.github_token = parse_value(&value),
            "paths.root" => self.paths.root = value.to_string(),
            "paths.packages" => self.paths.packages = value.to_string(),
            "paths.data" => self.paths.data = value.to_string(),
            "paths.logs" => self.paths.logs = value.to_string(),
            "paths.cache" => self.paths.cache = value.to_string(),
            "sources.primary" => self.sources.primary = value.to_string(),
            "sources.winget.url" => self.sources.winget.url = value.to_string(),
            "sources.winget.format" => self.sources.winget.format = value.to_string(),
            "sources.winget.manifest_kind" => self.sources.winget.manifest_kind = value.to_string(),
            "sources.winget.manifest_path_template" => {
                self.sources.winget.manifest_path_template = value.to_string()
            }
            "sources.winget.enabled" => self.sources.winget.enabled = parse_bool(key, &value)?,
            _ => return Err(anyhow!("unknown config key: {key}")),
        }

        Ok(())
    }

    fn load_cached() -> Result<Self> {
        let mut guard = lock_cache()?;

        if let Some(config) = guard.as_ref() {
            return Ok(config.clone());
        }

        let config = Self::load_default()?;
        *guard = Some(config.clone());
        Ok(config)
    }

    fn save_cached(config: &Self) -> Result<()> {
        config.save_default()?;
        update_cache(config.clone());
        Ok(())
    }
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load_cached()?;
    config.set_value(key, value)?;
    Config::save_cached(&config)
}

pub fn config_sections() -> Result<Vec<ConfigSection>> {
    Config::load_cached()?.effective_sections()
}

pub fn get_effective_value(key: &str) -> Result<(String, &'static str)> {
    Config::current().effective_value(key)
}

fn env_override(key: &str) -> Option<String> {
    env_override_names(key)
        .into_iter()
        .find_map(|name| env::var(&name).ok())
        .filter(|value| !value.trim().is_empty())
}

fn env_override_names(key: &str) -> Vec<String> {
    let mut names = vec![format!("WINBREW_{}", key.replace('.', "_").to_uppercase())];

    match key {
        "core.log_level" => names.push("WINBREW_LOG_LEVEL".to_string()),
        "core.auto_update" => names.push("WINBREW_AUTO_UPDATE".to_string()),
        "core.confirm_remove" => names.push("WINBREW_CONFIRM_REMOVE".to_string()),
        "core.default_yes" => names.push("WINBREW_DEFAULT_YES".to_string()),
        "core.color" => names.push("WINBREW_COLOR".to_string()),
        "core.download_timeout" => names.push("WINBREW_DOWNLOAD_TIMEOUT".to_string()),
        "core.concurrent_downloads" => {
            names.push("WINBREW_THREADS".to_string());
            names.push("WINBREW_CONCURRENT_DOWNLOADS".to_string());
        }
        "core.github_token" => names.push("WINBREW_GITHUB_TOKEN".to_string()),
        "core.proxy" => names.push("WINBREW_PROXY".to_string()),
        "paths.root" => names.push("WINBREW_ROOT".to_string()),
        "sources.primary" => names.push("WINBREW_PRIMARY_SOURCE".to_string()),
        "sources.winget.url" => names.push("WINBREW_REGISTRY_URL".to_string()),
        "sources.winget.format" => names.push("WINBREW_REGISTRY_FORMAT".to_string()),
        "sources.winget.manifest_kind" => names.push("WINBREW_MANIFEST_KIND".to_string()),
        "sources.winget.manifest_path_template" => {
            names.push("WINBREW_MANIFEST_PATH_TEMPLATE".to_string())
        }
        "sources.winget.enabled" => names.push("WINBREW_WINGET_ENABLED".to_string()),
        _ => {}
    }

    names
}

fn section_key(section_title: &str, key: &str) -> String {
    match section_title.to_lowercase().as_str() {
        "core" => format!("core.{key}"),
        "paths" => format!("paths.{key}"),
        "sources" => format!("sources.{key}"),
        _ => key.to_string(),
    }
}

fn validate_config_value(key: &str, value: &str) -> Result<()> {
    match key {
        "core.log_level" => {
            let normalized = value.trim().to_ascii_lowercase();
            let allowed_levels = ["trace", "debug", "info", "warn", "error"];

            if !allowed_levels.contains(&normalized.as_str()) {
                bail!("{key} must be one of: {}", allowed_levels.join(", "));
            }
        }
        "core.auto_update"
        | "core.confirm_remove"
        | "core.default_yes"
        | "core.color"
        | "sources.winget.enabled" => {
            value
                .parse::<bool>()
                .map_err(|_| anyhow!("{key} requires a boolean value (true or false)"))?;
        }
        "core.download_timeout" | "core.concurrent_downloads" => {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| anyhow!("{key} requires a whole number"))?;

            if parsed == 0 {
                return Err(anyhow!("{key} requires a positive number"));
            }
        }
        _ => {}
    }

    Ok(())
}

fn normalize_config_value(key: &str, value: &str) -> String {
    match key {
        "core.log_level" => value.trim().to_ascii_lowercase(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_winget_master_and_manifest_template() {
        let config = Config::default();

        assert_eq!(config.sources.winget.url, DEFAULT_REGISTRY_URL);
        assert_eq!(config.sources.winget.format, "yaml");
        assert_eq!(config.sources.winget.manifest_kind, "installer");
        assert_eq!(
            config.sources.winget.manifest_path_template,
            DEFAULT_WINGET_PATH_TEMPLATE
        );
        assert_eq!(config.paths.root, DEFAULT_ROOT);
    }

    #[test]
    fn resolved_paths_place_database_under_data_db() {
        let config = Config::default();
        let paths = config.resolved_paths();

        assert!(paths.db.ends_with(r"data\db\winbrew.db"));
        assert!(paths.config.ends_with(r"data\winbrew.toml"));
        assert!(paths.log.ends_with(r"data\logs\winbrew.log"));
    }

    #[test]
    fn get_and_set_trim_whitespace_around_keys_and_values() {
        let mut config = Config::default();

        config
            .set_value(" core.log_level ", " debug ")
            .expect("set_value should trim surrounding whitespace");

        assert_eq!(
            config
                .get_value(" core.log_level ")
                .expect("get_value should trim surrounding whitespace"),
            Some("debug".to_string())
        );
    }
}
