use crate::core::paths;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

mod errors;
mod keys;
mod lookup;
mod registry;
mod storage;
mod types;
mod validation;

pub use errors::ConfigError;
pub use storage::{config_sections, config_set, get_effective_value};
pub use types::*;

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
        let env = ConfigEnv::capture();
        let root = env
            .root_override()
            .map(str::to_owned)
            .unwrap_or_else(default_root_path);
        Ok(Self::load(&paths::config_file_at(Path::new(&root)))?.with_env(env))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = toml::to_string_pretty(self).context("failed to serialize config file")?;
        storage::atomic_write(path, &contents)
    }

    pub fn save_default(&self) -> Result<()> {
        let root = self
            .env
            .root_override()
            .map(str::to_owned)
            .or_else(|| {
                self.effective_value("paths.root")
                    .ok()
                    .map(|(value, _)| value)
            })
            .unwrap_or_else(default_root_path);

        self.save(&paths::config_file_at(Path::new(&root)))
    }

    pub fn load_current() -> Result<Self> {
        let env = ConfigEnv::capture();
        let root = env
            .root_override()
            .map(str::to_owned)
            .unwrap_or_else(default_root_path);

        Ok(Self::load(&paths::config_file_at(Path::new(&root)))?.with_env(env))
    }

    pub fn resolved_paths(&self) -> paths::ResolvedPaths {
        let root = self
            .effective_value("paths.root")
            .map(|(value, _)| value)
            .unwrap_or_else(|_| self.paths.root.clone());
        let root = std::path::PathBuf::from(root);
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
}
