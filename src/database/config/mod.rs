use crate::core::paths;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

mod keys;
mod lookup;
mod storage;
mod types;
mod validation;

pub use storage::{config_sections, config_set, get_effective_value};
pub use types::{Config, ConfigSection, CoreConfig, PathsConfig, SourceConfig, SourcesConfig};

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
        Self::load(&paths::config_file())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = toml::to_string_pretty(self).context("failed to serialize config file")?;
        storage::atomic_write(path, &contents)
    }

    pub fn save_default(&self) -> Result<()> {
        self.save(&paths::config_file())
    }

    pub fn current() -> Self {
        storage::load_cached().unwrap_or_default()
    }

    pub fn resolved_paths(&self) -> paths::ResolvedPaths {
        let root = std::path::PathBuf::from(&self.paths.root);
        paths::resolved_paths(
            &root,
            &self.paths.packages,
            &self.paths.data,
            &self.paths.logs,
            &self.paths.cache,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_winget_master_and_manifest_template() {
        let config = Config::default();

        assert_eq!(config.sources.winget.url, types::DEFAULT_REGISTRY_URL);
        assert_eq!(config.sources.winget.format, "yaml");
        assert_eq!(config.sources.winget.manifest_kind, "installer");
        assert_eq!(
            config.sources.winget.manifest_path_template,
            types::DEFAULT_WINGET_PATH_TEMPLATE
        );
        assert_eq!(config.paths.root, types::DEFAULT_ROOT);
        assert_eq!(
            config.core.file_log_level,
            "debug,winbrew::core::network=trace"
        );
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

        config
            .set_value(
                " core.file_log_level ",
                " warn,winbrew::core::network=trace ",
            )
            .expect("set_value should accept EnvFilter strings");

        assert_eq!(
            config
                .get_value(" core.log_level ")
                .expect("get_value should trim surrounding whitespace"),
            Some("debug".to_string())
        );

        assert_eq!(
            config
                .get_value(" core.file_log_level ")
                .expect("get_value should trim surrounding whitespace"),
            Some("warn,winbrew::core::network=trace".to_string())
        );
    }
}
