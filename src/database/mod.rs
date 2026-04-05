use anyhow::{Context, Result};
use r2d2::{Pool, PooledConnection};
use std::sync::{Mutex, OnceLock};

use crate::core::paths::ResolvedPaths;

mod catalog;
mod config;
mod connection;
mod errors;
mod installed_packages;
mod migration;

use self::connection::SqliteConnectionManager;

pub use errors::CatalogNotFoundError;

pub use catalog::{get_installers, search};
pub use config::{
    Config, ConfigEnv, ConfigError, ConfigSection, ConfigSource, CoreConfig, PathsConfig,
    config_sections, config_set, get_effective_value,
};
pub use installed_packages::{
    PackageNotFoundError, delete_package, get_package, insert_package, list_packages,
    update_status, update_status_and_msix_package_full_name,
};

static DB_POOL: OnceLock<Pool<SqliteConnectionManager>> = OnceLock::new();
static DB_POOL_INIT: Mutex<()> = Mutex::new(());
static CATALOG_DB_POOL: OnceLock<Pool<SqliteConnectionManager>> = OnceLock::new();
static CATALOG_DB_POOL_INIT: Mutex<()> = Mutex::new(());
static DB_PATHS: OnceLock<ResolvedPaths> = OnceLock::new();
static DB_PATHS_INIT: Mutex<()> = Mutex::new(());

pub fn init(paths: &ResolvedPaths) -> Result<()> {
    let _ = DB_PATHS.set(paths.clone());
    let _ = get_pool()?;

    Ok(())
}

fn resolved_paths() -> Result<&'static ResolvedPaths> {
    if let Some(paths) = DB_PATHS.get() {
        return Ok(paths);
    }

    let _guard = DB_PATHS_INIT
        .lock()
        .map_err(|_| anyhow::anyhow!("database paths init lock poisoned"))?;

    if let Some(paths) = DB_PATHS.get() {
        return Ok(paths);
    }

    let paths = Config::load_current()?.resolved_paths();

    let _ = DB_PATHS.set(paths);
    DB_PATHS
        .get()
        .context("failed to initialize database resolved paths")
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
        resolved_paths()?.db.clone(),
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
    if !resolved_paths()?.catalog_db.exists() {
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

    let pool = connection::build_pool(resolved_paths()?.catalog_db.clone(), true, 4, None)?;

    CATALOG_DB_POOL
        .set(pool)
        .map_err(|_| anyhow::anyhow!("catalog database pool was initialized concurrently"))?;

    CATALOG_DB_POOL
        .get()
        .context("failed to initialize catalog database connection pool")
}
