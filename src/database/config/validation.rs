use anyhow::{Context, Result, anyhow, bail};

use super::types::Config;

impl Config {
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let key = key.trim();

        if key.is_empty() {
            bail!("config key cannot be empty");
        }

        let value = value.trim();
        validate_config_value(key, value)?;
        let value = normalize_config_value(key, value);

        match key {
            "core.log_level" => self.core.log_level = value,
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
            "paths.root" => self.paths.root = value,
            "paths.packages" => self.paths.packages = value,
            "paths.data" => self.paths.data = value,
            "paths.logs" => self.paths.logs = value,
            "paths.cache" => self.paths.cache = value,
            "sources.primary" => self.sources.primary = value,
            "sources.winget.url" => self.sources.winget.url = value,
            "sources.winget.format" => self.sources.winget.format = value,
            "sources.winget.manifest_kind" => self.sources.winget.manifest_kind = value,
            "sources.winget.manifest_path_template" => {
                self.sources.winget.manifest_path_template = value
            }
            "sources.winget.enabled" => self.sources.winget.enabled = parse_bool(key, &value)?,
            _ => return Err(anyhow!("unknown config key: {key}")),
        }

        Ok(())
    }
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
