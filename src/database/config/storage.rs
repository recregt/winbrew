use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
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

fn update_cache(config: Config) {
    if let Ok(mut guard) = cache().lock() {
        *guard = Some(config);
    }
}

pub(crate) fn atomic_write(path: &Path, contents: &str) -> Result<()> {
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

pub(crate) fn load_cached() -> Result<Config> {
    let mut guard = lock_cache()?;

    if let Some(config) = guard.as_ref() {
        return Ok(config.clone());
    }

    let config = Config::load_default()?;
    *guard = Some(config.clone());
    Ok(config)
}

fn save_cached(config: &Config) -> Result<()> {
    config.save_default()?;
    update_cache(config.clone());
    Ok(())
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    let mut config = load_cached()?;
    config.set_value(key, value)?;
    save_cached(&config)
}

pub fn config_sections() -> Result<Vec<ConfigSection>> {
    Ok(load_cached()?.effective_sections()?)
}

pub fn get_effective_value(key: &str) -> Result<(String, &'static str)> {
    load_cached()?.effective_value(key)
}
