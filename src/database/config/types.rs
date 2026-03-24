use serde::{Deserialize, Serialize};

pub const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/microsoft/winget-pkgs/master";
pub const DEFAULT_WINGET_PATH_TEMPLATE: &str =
    "manifests/${partition}/${publisher}/${package}/${version}/${identifier}.${kind}.yaml";
pub const DEFAULT_ROOT: &str = r"C:\winbrew";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Env,
    File,
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

// Shared serde helper for bool fields that default to true.
fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_file_log_level() -> String {
    "debug,winbrew::core::network=trace".to_string()
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
