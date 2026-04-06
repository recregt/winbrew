use tracing_subscriber::EnvFilter;

use super::errors::ConfigValidationError;

pub struct KeyDef {
    pub key: &'static str,
    pub validator: Option<fn(&str) -> std::result::Result<(), ConfigValidationError>>,
}

pub static KEYS: &[KeyDef] = &[
    KeyDef {
        key: "core.log_level",
        validator: Some(|value| {
            let allowed = ["trace", "debug", "info", "warn", "error"];

            if !allowed.contains(&value.to_ascii_lowercase().as_str()) {
                return Err(ConfigValidationError::InvalidLogLevel {
                    value: value.to_string(),
                    allowed: allowed.join(", "),
                });
            }

            Ok(())
        }),
    },
    KeyDef {
        key: "core.file_log_level",
        validator: Some(|value| {
            EnvFilter::try_new(value).map_err(|err| {
                ConfigValidationError::InvalidFileLogLevel {
                    value: value.to_string(),
                    reason: err.to_string(),
                }
            })?;
            Ok(())
        }),
    },
    KeyDef {
        key: "core.auto_update",
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "core.confirm_remove",
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "core.default_yes",
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "core.color",
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "paths.root",
        validator: None,
    },
    KeyDef {
        key: "paths.packages",
        validator: None,
    },
    KeyDef {
        key: "paths.data",
        validator: None,
    },
    KeyDef {
        key: "paths.logs",
        validator: None,
    },
    KeyDef {
        key: "paths.cache",
        validator: None,
    },
];

pub fn find(key: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|def| def.key == key)
}

fn validate_bool(value: &str) -> std::result::Result<(), ConfigValidationError> {
    match value {
        "true" | "false" | "1" | "0" | "yes" | "no" | "on" | "off" => Ok(()),
        _ => Err(ConfigValidationError::ExpectedBoolean {
            value: value.to_string(),
        }),
    }
}
