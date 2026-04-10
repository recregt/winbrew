use super::errors::{ConfigError, ConfigResult};

use super::{registry, types::Config};

impl Config {
    pub fn set_value(&mut self, key: &str, value: &str) -> ConfigResult<()> {
        let key = key.trim();

        if key.is_empty() {
            return Err(ConfigError::EmptyKey);
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
            "paths.root" => self.paths.root = value,
            "paths.packages" => self.paths.packages = value,
            "paths.data" => self.paths.data = value,
            "paths.logs" => self.paths.logs = value,
            "paths.cache" => self.paths.cache = value,
            _ => {
                return Err(ConfigError::UnknownKey {
                    key: key.to_string(),
                });
            }
        }

        Ok(())
    }

    pub fn unset_value(&mut self, key: &str) -> ConfigResult<()> {
        let key = key.trim();

        if key.is_empty() {
            return Err(ConfigError::EmptyKey);
        }

        let defaults = Config::default();

        match key {
            "core.log_level" => self.core.log_level = defaults.core.log_level,
            "core.file_log_level" => self.core.file_log_level = defaults.core.file_log_level,
            "core.auto_update" => self.core.auto_update = defaults.core.auto_update,
            "core.confirm_remove" => self.core.confirm_remove = defaults.core.confirm_remove,
            "core.default_yes" => self.core.default_yes = defaults.core.default_yes,
            "core.color" => self.core.color = defaults.core.color,
            "paths.root" => self.paths.root = defaults.paths.root,
            "paths.packages" => self.paths.packages = defaults.paths.packages,
            "paths.data" => self.paths.data = defaults.paths.data,
            "paths.logs" => self.paths.logs = defaults.paths.logs,
            "paths.cache" => self.paths.cache = defaults.paths.cache,
            _ => {
                return Err(ConfigError::UnknownKey {
                    key: key.to_string(),
                });
            }
        }

        Ok(())
    }
}

fn parse_bool(key: &str, value: &str) -> ConfigResult<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        value => Err(ConfigError::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
        }),
    }
}

fn validate_config_value(key: &str, value: &str) -> ConfigResult<()> {
    if let Some(def) = registry::find(key) {
        if let Some(validator) = def.validator {
            if let Err(source) = validator(value) {
                return Err(ConfigError::Validation {
                    key: key.to_string(),
                    source,
                });
            }

            return Ok(());
        }

        return Ok(());
    }

    Err(ConfigError::UnknownKey {
        key: key.to_string(),
    })
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

    #[test]
    fn unset_value_restores_default_values() {
        let mut config = Config::default();

        config.set_value("core.auto_update", "false").unwrap();
        config.set_value("paths.packages", "C:\\custom").unwrap();

        config.unset_value("core.auto_update").unwrap();
        config.unset_value("paths.packages").unwrap();

        let defaults = Config::default();

        assert_eq!(config.core.auto_update, defaults.core.auto_update);
        assert_eq!(config.paths.packages, defaults.paths.packages);
    }
}
