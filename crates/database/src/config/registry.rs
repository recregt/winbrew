use tracing_subscriber::EnvFilter;

use super::error::ConfigValidationError;

pub type Validator = fn(&str) -> std::result::Result<(), ConfigValidationError>;

pub struct KeyDef {
    pub key: &'static str,
    pub validator: Option<Validator>,
}

pub static KEYS: &[KeyDef] = &[
    KeyDef {
        key: "core.log_level",
        validator: Some(|value| {
            EnvFilter::try_new(value).map_err(|err| ConfigValidationError::InvalidLogLevel {
                value: value.to_string(),
                reason: err.to_string(),
            })?;
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

pub fn suggest_key(key: &str) -> Option<&'static str> {
    let key = key.trim();

    if key.is_empty() {
        return None;
    }

    if let Some(exact) = KEYS.iter().find(|def| def.key.eq_ignore_ascii_case(key)) {
        return Some(exact.key);
    }

    if key.contains('.') {
        return None;
    }

    for section in ["core", "paths"] {
        let candidate = format!("{section}.{key}");

        if let Some(exact) = KEYS
            .iter()
            .find(|def| def.key.eq_ignore_ascii_case(&candidate))
        {
            return Some(exact.key);
        }
    }

    None
}

fn validate_bool(value: &str) -> std::result::Result<(), ConfigValidationError> {
    parse_bool_value(value)
        .map(|_| ())
        .ok_or_else(|| ConfigValidationError::ExpectedBoolean {
            value: value.to_string(),
        })
}

pub(super) fn parse_bool_value(value: &str) -> Option<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}
