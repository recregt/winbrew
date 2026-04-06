use std::collections::HashMap;

use super::keys::env_override;
use super::registry;
pub use crate::models::config::{ConfigSection, ConfigValueSource as ConfigSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub core: CoreConfig,

    #[serde(default)]
    pub paths: PathsConfig,

    #[serde(skip, default)]
    pub env: ConfigEnv,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigEnv {
    values: HashMap<String, String>,
}

impl ConfigEnv {
    pub fn capture() -> Self {
        let mut values = HashMap::new();

        for def in registry::KEYS {
            if let Some(value) = env_override(def.key) {
                values.insert(def.key.to_string(), value);
            }
        }

        Self { values }
    }

    pub fn value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn root_override(&self) -> Option<&str> {
        self.value("paths.root")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_file_log_level")]
    pub file_log_level: String,

    #[serde(default = "default_true")]
    pub auto_update: bool,

    #[serde(default = "default_true")]
    pub confirm_remove: bool,

    #[serde(default)]
    pub default_yes: bool,

    #[serde(default = "default_true")]
    pub color: bool,
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

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            file_log_level: default_file_log_level(),
            auto_update: default_true(),
            confirm_remove: default_true(),
            default_yes: false,
            color: default_true(),
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

// Shared serde helper for bool fields that default to true.
fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_file_log_level() -> String {
    "debug,winbrew::core=trace".to_string()
}

pub fn default_root_path() -> String {
    let local_app_data =
        std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA must be set on Windows");

    std::path::PathBuf::from(local_app_data)
        .join("winbrew")
        .to_string_lossy()
        .to_string()
}

// These path defaults are templates, not final paths.
// They are expanded from the configured root in core::paths::resolve_template.
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
