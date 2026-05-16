use super::error::{ConfigError, ConfigResult};

use super::{registry, types::Config};

impl Config {
    pub fn set_value(&mut self, key: &str, value: &str) -> ConfigResult<()> {
        let key = key.trim();

        if key.is_empty() {
            return Err(ConfigError::EmptyKey);
        }

        let value = value.trim();
        validate_config_value(key, value)?;
        let value = value.to_string();

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

        ensure_known_key(key)?;

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
    registry::parse_bool_value(value).ok_or_else(|| ConfigError::InvalidValue {
        key: key.to_string(),
        value: value.to_string(),
    })
}

fn validate_config_value(key: &str, value: &str) -> ConfigResult<()> {
    let def = ensure_known_key(key)?;

    if let Some(validator) = def.validator {
        validator(value).map_err(|source| ConfigError::Validation {
            key: key.to_string(),
            source,
        })?;
    }

    Ok(())
}

fn ensure_known_key(key: &str) -> ConfigResult<&'static registry::KeyDef> {
    registry::find(key).ok_or_else(|| ConfigError::UnknownKey {
        key: key.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};

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
    fn set_value_accepts_env_filter_syntax_for_console_log_level() {
        let mut config = Config::default();

        config
            .set_value("core.log_level", "winbrew=debug,info")
            .unwrap();

        assert_eq!(config.core.log_level, "winbrew=debug,info");
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

    #[test]
    fn unset_value_rejects_unknown_keys() {
        let mut config = Config::default();

        let err = config.unset_value("core.proxy").unwrap_err();

        assert!(matches!(err, ConfigError::UnknownKey { .. }));
    }
}
