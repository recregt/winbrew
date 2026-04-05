use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process;

use super::types::{Config, ConfigSection};

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

pub fn config_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load_default()?;

    config.set_value(key, value)?;
    config.save_default()?;
    Ok(())
}

pub fn config_sections() -> Result<Vec<ConfigSection>> {
    Config::load_current()?.effective_sections()
}

pub fn get_effective_value(key: &str) -> Result<(String, super::types::ConfigSource)> {
    Ok(Config::load_current()?.effective_value(key)?)
}
