use anyhow::{Context, Result, anyhow, bail};

use super::{registry, types::Config};

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
            "core.file_log_level" => self.core.file_log_level = value,
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
    if let Some(def) = registry::find(key) {
        if let Some(validator) = def.validator {
            return validator(value).with_context(|| format!("invalid value for '{key}'"));
        }

        return Ok(());
    }

    Err(anyhow!("unknown config key: {key}"))
}

fn normalize_config_value(key: &str, value: &str) -> String {
    match key {
        "core.log_level" => value.trim().to_ascii_lowercase(),
        "core.file_log_level" => value.trim().to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn set_value_accepts_boolean_aliases_in_validation() {
        let mut config = Config::default();

        config.set_value("core.auto_update", "yes").unwrap();
        config.set_value("core.confirm_remove", "on").unwrap();
        config.set_value("core.default_yes", "1").unwrap();

        assert!(config.core.auto_update);
        assert!(config.core.confirm_remove);
        assert!(config.core.default_yes);
    }
}
