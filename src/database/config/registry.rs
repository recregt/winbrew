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
        key: "core.download_timeout",
        env_aliases: &["WINBREW_DOWNLOAD_TIMEOUT"],
        validator: Some(validate_positive_u64),
    },
    KeyDef {
        key: "core.concurrent_downloads",
        env_aliases: &["WINBREW_THREADS", "WINBREW_CONCURRENT_DOWNLOADS"],
        validator: Some(validate_positive_u64),
    },
    KeyDef {
        key: "core.github_token",
        env_aliases: &["WINBREW_GITHUB_TOKEN"],
        validator: None,
    },
    KeyDef {
        key: "core.proxy",
        env_aliases: &["WINBREW_PROXY"],
        validator: None,
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
    KeyDef {
        key: "sources.primary",
        env_aliases: &["WINBREW_PRIMARY_SOURCE"],
        validator: None,
    },
    KeyDef {
        key: "sources.winget.repo_slug",
        env_aliases: &["WINBREW_WINGET_REPO_SLUG"],
        validator: None,
    },
    KeyDef {
        key: "sources.winget.api_base",
        env_aliases: &["WINBREW_GITHUB_API_BASE", "WINBREW_WINGET_API_BASE"],
        validator: None,
    },
    KeyDef {
        key: "sources.winget.url",
        env_aliases: &["WINBREW_REGISTRY_URL"],
        validator: None,
    },
    KeyDef {
        key: "sources.winget.format",
        env_aliases: &["WINBREW_REGISTRY_FORMAT"],
        validator: None,
    },
    KeyDef {
        key: "sources.winget.manifest_kind",
        env_aliases: &["WINBREW_MANIFEST_KIND"],
        validator: None,
    },
    KeyDef {
        key: "sources.winget.manifest_path_template",
        env_aliases: &["WINBREW_MANIFEST_PATH_TEMPLATE"],
        validator: None,
    },
    KeyDef {
        key: "sources.winget.enabled",
        env_aliases: &["WINBREW_WINGET_ENABLED"],
        validator: Some(validate_bool),
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

fn validate_positive_u64(value: &str) -> Result<()> {
    match value.parse::<u64>() {
        Ok(0) => Err(anyhow!("value must be greater than zero")),
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow!("expected a positive whole number")),
    }
}
