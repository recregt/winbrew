use anyhow::{Context, Result, anyhow};
use dashmap::DashMap;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use super::lock_conn;

static CONFIG_CACHE: OnceLock<DashMap<String, Arc<ConfigCell>>> = OnceLock::new();

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

fn config_cache() -> &'static DashMap<String, Arc<ConfigCell>> {
    CONFIG_CACHE.get_or_init(DashMap::default)
}

fn config_cache_cell(key: &str) -> Arc<ConfigCell> {
    config_cache()
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(ConfigCell::new()))
        .clone()
}

fn load_config_from_db(key: &str) -> Result<Option<String>> {
    let conn = lock_conn()?;
    config_string(&conn, key)
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

                let loaded = load_config_from_db(key);

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

pub fn config_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .context("failed to set config")?;

    let cell = config_cache_cell(key);
    if let Ok(mut state) = cell.state.lock() {
        *state = ConfigState::Ready(Some(value.to_string()));
        cell.ready.notify_all();
    }

    Ok(())
}

pub fn config_list(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT key, value FROM config ORDER BY key ASC")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

    let mut pairs = Vec::new();
    for row in rows {
        pairs.push(row?);
    }

    Ok(pairs)
}

pub fn config_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM config WHERE key = ?1")?;
    stmt.query_row(params![key], |row| row.get(0))
        .optional()
        .context("failed to get config")
}

pub fn config_string(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(config_get(conn, key)?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

pub fn config_u64(conn: &Connection, key: &str) -> Result<Option<u64>> {
    parse_config_value(config_string(conn, key)?.as_deref(), |value| {
        value
            .parse::<u64>()
            .with_context(|| format!("invalid {key} value"))
    })
}

pub fn config_bool(conn: &Connection, key: &str) -> Result<Option<bool>> {
    parse_config_value(config_string(conn, key)?.as_deref(), |value| {
        parse_bool(key, value)
    })
}
