use anyhow::{Context, Result, anyhow};
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::core::paths;

mod config;
mod packages;

pub use config::{
    clear_config_cache, config_bool, config_bool_cached, config_get, config_list, config_set,
    config_string, config_string_cached, config_u64, config_u64_cached,
};
pub use packages::{delete_package, get_package, insert_package, list_packages, update_status};

static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

pub fn connect() -> Result<Connection> {
    let path = paths::db_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create winbrew data directory")?;
    }

    let conn = Connection::open(&path).context("failed to open database")?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
    )
    .context("failed to set pragmas")?;

    Ok(conn)
}

pub fn get_conn() -> Result<&'static Mutex<Connection>> {
    if let Some(conn) = DB_CONN.get() {
        return Ok(conn);
    }

    let conn = connect()?;
    migrate(&conn)?;

    let _ = DB_CONN.set(Mutex::new(conn));

    DB_CONN
        .get()
        .context("failed to initialize database connection")
}

pub fn lock_conn() -> Result<MutexGuard<'static, Connection>> {
    let conn = get_conn()?;
    conn.lock()
        .map_err(|_| anyhow!("database connection lock poisoned"))
}

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS packages (
            name         TEXT PRIMARY KEY,
            version      TEXT NOT NULL,
            kind         TEXT NOT NULL,
            install_dir  TEXT NOT NULL,
            shims        TEXT NOT NULL DEFAULT '[]',
            dependencies TEXT NOT NULL DEFAULT '[]',
            status       TEXT NOT NULL DEFAULT 'installing',
            installed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS config (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ",
    )
    .context("migration failed")?;

    Ok(())
}
