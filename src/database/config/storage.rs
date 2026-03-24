use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process;
use std::sync::{Mutex, OnceLock};

use super::types::{Config, ConfigSection};

static CONFIG_CACHE: OnceLock<Mutex<Option<Config>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<Config>> {
    CONFIG_CACHE.get_or_init(|| Mutex::new(None))
}

fn lock_cache() -> Result<std::sync::MutexGuard<'static, Option<Config>>> {
    cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("config cache lock poisoned"))
}

pub(crate) fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }

    let temp_path = path.with_extension(format!("toml.{}.tmp", process::id()));
    {
        let mut file = fs::File::create(&temp_path).context("failed to create temp config file")?;
        file.write_all(contents.as_bytes())
            .context("failed to write temp config file")?;
        file.flush().context("failed to flush temp config file")?;
    }

    if let Err(err) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(err).context("failed to finalize config file");
    }
    Ok(())
}

pub(crate) fn load_cached() -> Result<Config> {
    let mut guard = lock_cache()?;

    if let Some(config) = guard.as_ref() {
        return Ok(config.clone());
    }

    let config = Config::load_default()?;
    *guard = Some(config.clone());
    Ok(config)
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    let mut guard = lock_cache()?;
    let mut config = match guard.as_ref() {
        Some(current) => current.clone(),
        None => Config::load_default()?,
    };

    config.set_value(key, value)?;
    config.save_default()?;
    *guard = Some(config);
    Ok(())
}

pub fn config_sections() -> Result<Vec<ConfigSection>> {
    load_cached()?.effective_sections()
}

pub fn get_effective_value(key: &str) -> Result<(String, super::types::ConfigSource)> {
    load_cached()?.effective_value(key)
}
