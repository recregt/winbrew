use anyhow::{Context, Result, anyhow, bail};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use crate::core::paths;

static CONFIG_CACHE: OnceLock<DashMap<String, Arc<ConfigCell>>> = OnceLock::new();
const SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigDocument {
    #[serde(default = "default_schema_version")]
    schema_version: u64,

    #[serde(default)]
    config: BTreeMap<String, toml::Value>,
}

struct ConfigCell {
    state: Mutex<ConfigState>,
    ready: Condvar,
}

enum ConfigState {
    Empty,
    Loading,
    Ready(Option<String>),
}

impl ConfigCell {
    fn new() -> Self {
        Self {
            state: Mutex::new(ConfigState::Empty),
            ready: Condvar::new(),
        }
    }
}

fn default_schema_version() -> u64 {
    SCHEMA_VERSION
}

fn config_cache() -> &'static DashMap<String, Arc<ConfigCell>> {
    CONFIG_CACHE.get_or_init(DashMap::default)
}

fn config_cache_cell(key: &str) -> Arc<ConfigCell> {
    config_cache()
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(ConfigCell::new()))
        .clone()
}

fn parse_config_value<T>(
    value: Option<&str>,
    parse: impl FnOnce(&str) -> Result<T>,
) -> Result<Option<T>> {
    match value {
        Some(value) => parse(value).map(Some),
        None => Ok(None),
    }
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        value => Err(anyhow!("invalid {key} value: {value}")),
    }
}

fn config_path() -> std::path::PathBuf {
    paths::config_file()
}

fn read_config_document() -> Result<ConfigDocument> {
    let path = config_path();

    match fs::read_to_string(&path) {
        Ok(content) => {
            if content.trim().is_empty() {
                Ok(ConfigDocument::default())
            } else {
                toml::from_str(&content).context("failed to parse config file")
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ConfigDocument::default()),
        Err(err) => Err(anyhow::Error::new(err).context("failed to read config file")),
    }
}

fn write_config_document(document: &ConfigDocument) -> Result<()> {
    let path = config_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }

    let serialized = toml::to_string_pretty(document).context("failed to serialize config file")?;
    let temp_path = path.with_extension("toml.tmp");

    {
        let mut file = fs::File::create(&temp_path).context("failed to create temp config file")?;
        file.write_all(serialized.as_bytes())
            .context("failed to write temp config file")?;
        file.flush().context("failed to flush temp config file")?;
    }

    if path.exists() {
        fs::remove_file(&path).context("failed to replace config file")?;
    }

    fs::rename(&temp_path, &path).context("failed to finalize config file")?;
    Ok(())
}

fn config_value_to_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(value) => Some(value.clone()),
        toml::Value::Integer(value) => Some(value.to_string()),
        toml::Value::Float(value) => Some(value.to_string()),
        toml::Value::Boolean(value) => Some(value.to_string()),
        toml::Value::Datetime(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_from_str(value: &str) -> toml::Value {
    if let Ok(boolean) = value.parse::<bool>() {
        toml::Value::Boolean(boolean)
    } else if let Ok(number) = value.parse::<i64>() {
        toml::Value::Integer(number)
    } else if let Ok(number) = value.parse::<f64>() {
        toml::Value::Float(number)
    } else {
        toml::Value::String(value.to_string())
    }
}

fn load_config_value(key: &str) -> Result<Option<String>> {
    let document = read_config_document()?;
    Ok(document
        .config
        .get(key)
        .and_then(config_value_to_string)
        .filter(|value| !value.trim().is_empty()))
}

pub fn config_string_cached(key: &str) -> Result<Option<String>> {
    let cell = config_cache_cell(key);

    loop {
        let mut state = cell
            .state
            .lock()
            .map_err(|_| anyhow!("config cache lock poisoned"))?;

        match &*state {
            ConfigState::Ready(value) => return Ok(value.clone()),
            ConfigState::Loading => {
                let _guard = cell
                    .ready
                    .wait(state)
                    .map_err(|_| anyhow!("config cache lock poisoned"))?;
                continue;
            }
            ConfigState::Empty => {
                *state = ConfigState::Loading;
                drop(state);

                let loaded = load_config_value(key);

                let mut state = cell
                    .state
                    .lock()
                    .map_err(|_| anyhow!("config cache lock poisoned"))?;

                match (&*state, loaded) {
                    (ConfigState::Loading, Ok(value)) => {
                        *state = ConfigState::Ready(value.clone());
                        cell.ready.notify_all();
                        return Ok(value);
                    }
                    (ConfigState::Loading, Err(err)) => {
                        *state = ConfigState::Empty;
                        cell.ready.notify_all();
                        return Err(err);
                    }
                    (ConfigState::Ready(value), _) => {
                        let value = value.clone();
                        cell.ready.notify_all();
                        return Ok(value);
                    }
                    (ConfigState::Empty, _) => continue,
                }
            }
        }
    }
}

pub fn config_bool_cached(key: &str) -> Result<Option<bool>> {
    parse_config_value(config_string_cached(key)?.as_deref(), |value| {
        parse_bool(key, value)
    })
}

pub fn config_u64_cached(key: &str) -> Result<Option<u64>> {
    parse_config_value(config_string_cached(key)?.as_deref(), |value| {
        value
            .parse::<u64>()
            .with_context(|| format!("invalid {key} value"))
    })
}

pub fn clear_config_cache() {
    if let Some(cache) = CONFIG_CACHE.get() {
        cache.clear();
    }
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    let clean_key = key.trim();
    if clean_key.is_empty() {
        bail!("config key cannot be empty");
    }

    let clean_value = value.trim();
    let mut document = read_config_document()?;
    document.schema_version = SCHEMA_VERSION;
    document
        .config
        .insert(clean_key.to_string(), value_from_str(clean_value));
    write_config_document(&document)?;

    let cell = config_cache_cell(clean_key);
    if let Ok(mut state) = cell.state.lock() {
        *state = ConfigState::Ready(Some(clean_value.to_string()));
        cell.ready.notify_all();
    }

    Ok(())
}

pub fn config_list() -> Result<Vec<(String, String)>> {
    let document = read_config_document()?;
    let mut pairs = document
        .config
        .into_iter()
        .filter_map(|(key, value)| config_value_to_string(&value).map(|value| (key, value)))
        .collect::<Vec<_>>();
    pairs.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(pairs)
}

pub fn config_get(key: &str) -> Result<Option<String>> {
    load_config_value(key)
}

pub fn config_string(key: &str) -> Result<Option<String>> {
    Ok(config_get(key)?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

pub fn config_u64(key: &str) -> Result<Option<u64>> {
    parse_config_value(config_string(key)?.as_deref(), |value| {
        value
            .parse::<u64>()
            .with_context(|| format!("invalid {key} value"))
    })
}

pub fn config_bool(key: &str) -> Result<Option<bool>> {
    parse_config_value(config_string(key)?.as_deref(), |value| {
        parse_bool(key, value)
    })
}
