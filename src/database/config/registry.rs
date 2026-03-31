use anyhow::{Result, anyhow, bail};
use tracing_subscriber::EnvFilter;

pub struct KeyDef {
    pub key: &'static str,
    pub env_aliases: &'static [&'static str],
    pub validator: Option<fn(&str) -> Result<()>>,
}

pub static KEYS: &[KeyDef] = &[
    KeyDef {
        key: "core.log_level",
        env_aliases: &["WINBREW_LOG_LEVEL"],
        validator: Some(|value| {
            let allowed = ["trace", "debug", "info", "warn", "error"];

            if !allowed.contains(&value.to_ascii_lowercase().as_str()) {
                bail!("core.log_level must be one of: {}", allowed.join(", "));
            }

            Ok(())
        }),
    },
    KeyDef {
        key: "core.file_log_level",
        env_aliases: &["WINBREW_FILE_LOG_LEVEL"],
        validator: Some(|value| {
            EnvFilter::try_new(value)
                .map_err(|err| anyhow!("invalid core.file_log_level: {err}"))?;
            Ok(())
        }),
    },
    KeyDef {
        key: "core.auto_update",
        env_aliases: &["WINBREW_AUTO_UPDATE"],
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "core.confirm_remove",
        env_aliases: &["WINBREW_CONFIRM_REMOVE"],
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "core.default_yes",
        env_aliases: &["WINBREW_DEFAULT_YES"],
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "core.color",
        env_aliases: &["WINBREW_COLOR"],
        validator: Some(validate_bool),
    },
    KeyDef {
        key: "paths.root",
        env_aliases: &["WINBREW_ROOT"],
        validator: None,
    },
    KeyDef {
        key: "paths.packages",
        env_aliases: &[],
        validator: None,
    },
    KeyDef {
        key: "paths.data",
        env_aliases: &[],
        validator: None,
    },
    KeyDef {
        key: "paths.logs",
        env_aliases: &[],
        validator: None,
    },
    KeyDef {
        key: "paths.cache",
        env_aliases: &[],
        validator: None,
    },
];

pub fn find(key: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|def| def.key == key)
}

fn validate_bool(value: &str) -> Result<()> {
    match value {
        "true" | "false" | "1" | "0" | "yes" | "no" | "on" | "off" => Ok(()),
        _ => Err(anyhow!("expected a boolean value (true/false)")),
    }
}
