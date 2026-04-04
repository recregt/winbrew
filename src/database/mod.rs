use anyhow::{Context, Result};
use r2d2::{Pool, PooledConnection};
use std::sync::{Mutex, OnceLock};

mod catalog;
mod config;
mod connection;
mod errors;
mod installed_packages;
mod migration;

use self::connection::SqliteConnectionManager;

pub use errors::CatalogNotFoundError;

pub use catalog::{get_installers, search};
pub(crate) use config::section_key;
pub use config::{
    Config, ConfigEnv, ConfigError, ConfigSection, ConfigSource, CoreConfig, PathsConfig,
    config_sections, config_set, get_effective_value,
};
pub use installed_packages::{
    PackageNotFoundError, delete_package, get_package, insert_package, list_packages, update_status,
};

static DB_POOL: OnceLock<Pool<SqliteConnectionManager>> = OnceLock::new();
static DB_POOL_INIT: Mutex<()> = Mutex::new(());
static CATALOG_DB_POOL: OnceLock<Pool<SqliteConnectionManager>> = OnceLock::new();
static CATALOG_DB_POOL_INIT: Mutex<()> = Mutex::new(());

pub fn init() -> Result<()> {
    let _ = get_pool()?;

    Ok(())
}

pub fn get_pool() -> Result<&'static Pool<SqliteConnectionManager>> {
    if let Some(pool) = DB_POOL.get() {
        return Ok(pool);
    }

    let _guard = DB_POOL_INIT
        .lock()
        .map_err(|_| anyhow::anyhow!("database pool init lock poisoned"))?;

    if let Some(pool) = DB_POOL.get() {
        return Ok(pool);
    }

    let pool = connection::build_pool(
        Config::current().resolved_paths().db,
        false,
        10,
        Some(migration::migrate),
    )?;

    DB_POOL
        .set(pool)
        .map_err(|_| anyhow::anyhow!("database pool was initialized concurrently"))?;

    DB_POOL
        .get()
        .context("failed to initialize database connection pool")
}

pub fn get_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    let pool = get_pool()?;
    pool.get()
        .context("failed to acquire database connection from pool")
}

pub fn get_catalog_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    let catalog_db = Config::current().resolved_paths().catalog_db;

    if !catalog_db.exists() {
        return Err(CatalogNotFoundError.into());
    }

    let pool = get_catalog_pool()?;
    pool.get()
        .context("failed to acquire catalog database connection from pool")
}

pub fn get_catalog_pool() -> Result<&'static Pool<SqliteConnectionManager>> {
    if let Some(pool) = CATALOG_DB_POOL.get() {
        return Ok(pool);
    }

    let _guard = CATALOG_DB_POOL_INIT
        .lock()
        .map_err(|_| anyhow::anyhow!("catalog database pool init lock poisoned"))?;

    if let Some(pool) = CATALOG_DB_POOL.get() {
        return Ok(pool);
    }

    let catalog_db = Config::current().resolved_paths().catalog_db;
    let pool = connection::build_pool(catalog_db, true, 4, None)?;

    CATALOG_DB_POOL
        .set(pool)
        .map_err(|_| anyhow::anyhow!("catalog database pool was initialized concurrently"))?;

    CATALOG_DB_POOL
        .get()
        .context("failed to initialize catalog database connection pool")
}
