use anyhow::{Context, Result};
use r2d2::{Pool, PooledConnection};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::core::paths::ResolvedPaths;

mod catalog;
mod config;
mod connection;
mod errors;
mod installed_packages;
mod journal;
mod migration;

use self::connection::SqliteConnectionManager;

pub type DbConnection = PooledConnection<SqliteConnectionManager>;

pub use errors::CatalogNotFoundError;

pub use catalog::{get_installers, get_package_by_id, search};
pub use config::{
    Config, ConfigEnv, ConfigError, ConfigSection, ConfigSource, ConfigValidationError, CoreConfig,
    PathsConfig, config_sections, config_set, config_unset, get_effective_value,
};
pub use installed_packages::{
    PackageNotFoundError, delete_package, get_package, insert_package, list_installing_packages,
    list_packages, update_status, update_status_and_engine_metadata,
};
pub use journal::{
    FileHash, HashAlgo, JournalEntry, JournalReadError, JournalReader, JournalWriter,
};

thread_local! {
    static CURRENT_PATHS: RefCell<Option<ResolvedPaths>> = const { RefCell::new(None) };
}

static DB_POOLS: OnceLock<Mutex<HashMap<PathBuf, &'static Pool<SqliteConnectionManager>>>> =
    OnceLock::new();
static CATALOG_DB_POOLS: OnceLock<Mutex<HashMap<PathBuf, &'static Pool<SqliteConnectionManager>>>> =
    OnceLock::new();

pub fn init(paths: &ResolvedPaths) -> Result<()> {
    CURRENT_PATHS.with(|current_paths| {
        *current_paths.borrow_mut() = Some(paths.clone());
    });

    let _ = get_pool()?;

    Ok(())
}

fn resolved_paths() -> Result<ResolvedPaths> {
    CURRENT_PATHS.with(|current_paths| {
        if current_paths.borrow().is_none() {
            let paths = Config::load_current()?.resolved_paths();
            *current_paths.borrow_mut() = Some(paths);
        }

        current_paths
            .borrow()
            .as_ref()
            .cloned()
            .context("failed to initialize database resolved paths")
    })
}

pub fn get_pool() -> Result<&'static Pool<SqliteConnectionManager>> {
    pool_for(
        DB_POOLS.get_or_init(|| Mutex::new(HashMap::new())),
        resolved_paths()?.db.clone(),
        false,
        10,
        Some(migration::migrate),
    )
}

pub fn get_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    let pool = get_pool()?;
    pool.get()
        .context("failed to acquire database connection from pool")
}

pub fn get_catalog_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    if !resolved_paths()?.catalog_db.exists() {
        return Err(CatalogNotFoundError.into());
    }

    let pool = get_catalog_pool()?;
    pool.get()
        .context("failed to acquire catalog database connection from pool")
}

pub fn get_catalog_pool() -> Result<&'static Pool<SqliteConnectionManager>> {
    pool_for(
        CATALOG_DB_POOLS.get_or_init(|| Mutex::new(HashMap::new())),
        resolved_paths()?.catalog_db.clone(),
        true,
        4,
        None,
    )
}

fn pool_for(
    pools: &'static Mutex<HashMap<PathBuf, &'static Pool<SqliteConnectionManager>>>,
    path: PathBuf,
    read_only: bool,
    max_size: u32,
    migrate: Option<fn(&rusqlite::Connection) -> Result<()>>,
) -> Result<&'static Pool<SqliteConnectionManager>> {
    let mut pools = pools
        .lock()
        .map_err(|_| anyhow::anyhow!("database pool registry lock poisoned"))?;

    if let Some(pool) = pools.get(&path) {
        return Ok(*pool);
    }

    let pool = Box::leak(Box::new(connection::build_pool(
        path.clone(),
        read_only,
        max_size,
        migrate,
    )?)) as &'static Pool<SqliteConnectionManager>;

    pools.insert(path, pool);
    Ok(pool)
}
