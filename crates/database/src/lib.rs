//! Persistence layer for WinBrew.
//!
//! `winbrew-database` owns SQLite access, config persistence, journal replay,
//! and MSI inventory normalization. It stays close to the runtime database
//! contract so higher layers can use typed helpers instead of direct SQL
//! plumbing.
//!
//! The database module keeps its pool registry keyed by resolved paths. That
//! makes the current process-local root model explicit while still keeping the
//! storage boundary centralized for the app and CLI layers.

#![cfg(windows)]

pub use winbrew_core as core;

pub mod catalog;
pub mod config;
pub mod connection;
pub mod error;
pub mod installed_packages;
pub mod journal;
pub mod migration;
pub mod msi_inventory;

use self::connection::SqliteConnectionManager;
use anyhow::{Context, Result};
use r2d2::{Pool, PooledConnection};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use winbrew_core::paths::ResolvedPaths;

pub type DbConnection = PooledConnection<SqliteConnectionManager>;

pub use error::{CatalogNotFoundError, CatalogSchemaVersionMismatchError};

pub use catalog::{get_installers, get_package_by_id, search};
pub use config::{
    Config, ConfigEnv, ConfigError, ConfigSection, ConfigSource, ConfigValidationError, CoreConfig,
    PathsConfig, config_sections, config_set, config_unset, get_effective_value,
};
pub use installed_packages::{
    PackageNotFoundError, commit_install, delete_package, get_package, insert_package,
    list_installing_packages, list_packages, replay_committed_journal, update_installing_identity,
    update_status, update_status_and_engine_metadata,
};
pub use journal::{
    CommittedJournalPackage, FileHash, HashAlgo, JournalEntry, JournalReadError, JournalReader,
    JournalReplayError, JournalWriter,
};
pub use msi_inventory::{
    apply_snapshot, find_packages_by_normalized_path,
    find_packages_by_normalized_registry_key_path, get_snapshot, replace_snapshot, upsert_receipt,
};

thread_local! {
    static CURRENT_PATHS: RefCell<Option<ResolvedPaths>> = const { RefCell::new(None) };
}

static DB_POOLS: OnceLock<Mutex<HashMap<PathBuf, &'static Pool<SqliteConnectionManager>>>> =
    OnceLock::new();
static CATALOG_DB_POOLS: OnceLock<Mutex<HashMap<PathBuf, &'static Pool<SqliteConnectionManager>>>> =
    OnceLock::new();

/// Initialize the process-local storage state for the given resolved paths.
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

/// Return the primary database connection pool.
pub fn get_pool() -> Result<&'static Pool<SqliteConnectionManager>> {
    pool_for(
        DB_POOLS.get_or_init(|| Mutex::new(HashMap::new())),
        resolved_paths()?.db.clone(),
        false,
        10,
        Some(migration::migrate),
    )
}

/// Return a pooled connection to the primary database.
pub fn get_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    let pool = get_pool()?;
    pool.get()
        .context("failed to acquire database connection from pool")
}

/// Return a pooled connection to the catalog database.
pub fn get_catalog_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    if !resolved_paths()?.catalog_db.exists() {
        return Err(CatalogNotFoundError.into());
    }

    let pool = get_catalog_pool()?;
    let conn = pool
        .get()
        .context("failed to acquire catalog database connection from pool")?;
    catalog::ensure_schema_version(&conn)?;

    Ok(conn)
}

/// Return the catalog database connection pool.
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
