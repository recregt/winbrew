use crate::core::{fs::atomic_write_toml_temp, paths};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

mod error;
mod keys;
mod lookup;
mod registry;
mod storage;
mod types;
mod validation;

pub use error::{ConfigError, ConfigValidationError};
pub use storage::{config_sections, config_set, config_unset, get_effective_value};
pub use types::*;

pub fn suggest_key(key: &str) -> Option<&'static str> {
    registry::suggest_key(key)
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

    pub fn load_at(root: &Path) -> Result<Self> {
        let mut config = Self::load(&paths::config_file_at(root))?;
        config.paths.root = root
            .to_str()
            .context("config root path is not valid UTF-8")?
            .to_owned();

        // Explicit-root loads are used by tests and isolated stores, so they
        // intentionally ignore ambient WINBREW_* overrides.
        Ok(config.with_env(ConfigEnv::default()).with_config_root(root))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = toml::to_string_pretty(self).context("failed to serialize config file")?;
        Ok(atomic_write_toml_temp(path, &contents)?)
    }

    /// Save the config back to the root it was loaded from.
    ///
    /// For [`Config::load_current`], this is the environment/default-selected
    /// config storage root. For [`Config::load_at`], this remains the explicit
    /// root passed by the caller, so ambient `WINBREW_*` overrides stay ignored.
    /// This storage root can differ from the effective runtime `paths.root`.
    pub fn save_default(&self) -> Result<()> {
        let root = self.config_storage_root();
        let config_path = paths::config_file_at(&root);
        self.save(&config_path)
    }

    pub fn load_current() -> Result<Self> {
        let env = ConfigEnv::capture();
        let root = Self::resolve_root_from_env(&env);
        let config_path = paths::config_file_at(&root);
        Ok(Self::load(&config_path)?
            .with_env(env)
            .with_config_root(&root))
    }

    /// Build runtime paths from the effective config values.
    ///
    /// This uses effective `paths.root`, including environment overrides. That
    /// is intentionally separate from the config storage root used by
    /// [`Config::save_default`].
    pub fn resolved_paths(&self) -> paths::ResolvedPaths {
        let root = self.runtime_root();
        paths::resolved_paths(
            &root,
            &self.paths.packages,
            &self.paths.data,
            &self.paths.logs,
            &self.paths.cache,
        )
    }

    fn with_env(mut self, env: ConfigEnv) -> Self {
        self.env = env;
        self
    }

    fn with_config_root(mut self, root: &Path) -> Self {
        self.config_root = Some(root.to_path_buf());
        self
    }

    fn config_storage_root(&self) -> PathBuf {
        self.config_root
            .clone()
            .unwrap_or_else(|| Self::resolve_root_from_env(&self.env))
    }

    fn runtime_root(&self) -> PathBuf {
        self.effective_value("paths.root")
            .map(|(value, _)| PathBuf::from(value))
            .unwrap_or_else(|err| {
                warn!(
                    error = %err,
                    "effective_value(\"paths.root\") failed, using raw config field as fallback"
                );
                PathBuf::from(&self.paths.root)
            })
    }

    fn resolve_root_from_env(env: &ConfigEnv) -> PathBuf {
        env.root_override()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(default_root_path()))
    }
}
