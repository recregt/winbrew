use anyhow::{Context, Result};
use r2d2::{ManageConnection, Pool, PooledConnection};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use crate::core::paths;

mod config;
mod health;
mod packages;

pub use config::{
    Config, ConfigSection, CoreConfig, PathsConfig, SourceConfig, SourcesConfig, config_sections,
    config_set, get_effective_value,
};
pub use health::{
    HealthReport, ReportSection, RuntimeReport, get_health_report, get_runtime_report,
};
pub use packages::{delete_package, get_package, insert_package, list_packages, update_status};

static DB_POOL: OnceLock<Pool<SqliteConnectionManager>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct SqliteConnectionManager {
    path: PathBuf,
}

impl SqliteConnectionManager {
    fn file(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ManageConnection for SqliteConnectionManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> std::result::Result<Self::Connection, Self::Error> {
        open_connection(&self.path)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> std::result::Result<(), Self::Error> {
        conn.execute_batch("SELECT 1;")
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

pub fn connect() -> Result<Connection> {
    open_connection(&paths::db_path()).context("failed to open database")
}

pub fn init() -> Result<()> {
    let _ = pool()?;

    Ok(())
}

pub fn get_pool() -> Result<&'static Pool<SqliteConnectionManager>> {
    if let Some(pool) = DB_POOL.get() {
        return Ok(pool);
    }

    if let Some(parent) = paths::db_path().parent() {
        std::fs::create_dir_all(parent).context("failed to create winbrew database directory")?;
    }

    let manager = SqliteConnectionManager::file(paths::db_path());
    let pool = Pool::builder()
        .max_size(10)
        .build(manager)
        .context("failed to initialize SQLite connection pool")?;

    let conn = pool
        .get()
        .context("failed to get database connection for migrations")?;
    migrate(&conn)?;

    let _ = DB_POOL.set(pool);

    DB_POOL
        .get()
        .context("failed to initialize database connection pool")
}

pub fn get_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    let pool = get_pool()?;
    pool.get()
        .context("failed to acquire database connection from pool")
}

pub fn lock_conn() -> Result<PooledConnection<SqliteConnectionManager>> {
    get_conn()
}

fn pool() -> Result<&'static Pool<SqliteConnectionManager>> {
    get_pool()
}

fn open_connection(path: &Path) -> std::result::Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
    )?;

    Ok(conn)
}

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS packages (
            name         TEXT PRIMARY KEY,
            version      TEXT NOT NULL,
            kind         TEXT NOT NULL,
            install_dir  TEXT NOT NULL,
            product_code TEXT,
            dependencies TEXT NOT NULL DEFAULT '[]',
            status       TEXT NOT NULL DEFAULT 'installing',
            installed_at TEXT NOT NULL
        );
    ",
    )
    .context("migration failed")?;

    if !table_has_column(conn, "packages", "product_code")? {
        conn.execute_batch("ALTER TABLE packages ADD COLUMN product_code TEXT;")
            .context("failed to add product_code column")?;
    }

    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .context("failed to inspect table schema")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .context("failed to read table schema")?;

    for entry in columns {
        if entry.context("failed to read schema row")? == column {
            return Ok(true);
        }
    }

    Ok(false)
}
