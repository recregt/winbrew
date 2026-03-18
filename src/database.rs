#![allow(dead_code)]

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::core::paths;

static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shim {
    pub name: String,
    pub path: String,
    pub args: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PackageStatus {
    Installing,
    Ok,
    Updating,
    Failed,
}

impl PackageStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Installing => "installing",
            Self::Ok => "ok",
            Self::Updating => "updating",
            Self::Failed => "failed",
        }
    }

    fn from_str(status: &str) -> Self {
        match status {
            "ok" => Self::Ok,
            "updating" => Self::Updating,
            "failed" => Self::Failed,
            _ => Self::Installing,
        }
    }
}

impl std::fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub kind: String,
    pub install_dir: String,
    pub shims: Vec<Shim>,
    pub dependencies: Vec<String>,
    pub status: PackageStatus,
    pub installed_at: String,
}

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

    DB_CONN.get().context("failed to initialize database connection")
}

pub fn lock_conn() -> Result<MutexGuard<'static, Connection>> {
    let conn = get_conn()?;
    conn.lock()
        .map_err(|_| anyhow!("database connection lock poisoned"))
}

pub fn migrate(conn: &Connection) -> Result<()> {
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

pub fn insert_package(conn: &Connection, pkg: &Package) -> Result<()> {
    let shims = serde_json::to_string(&pkg.shims).context("failed to serialize shims")?;
    let deps =
        serde_json::to_string(&pkg.dependencies).context("failed to serialize dependencies")?;

    conn.execute(
        "INSERT OR REPLACE INTO packages
         (name, version, kind, install_dir, shims, dependencies, status, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            pkg.name,
            pkg.version,
            pkg.kind,
            pkg.install_dir,
            shims,
            deps,
            pkg.status.as_str(),
            pkg.installed_at,
        ],
    )
    .context("failed to insert package")?;

    Ok(())
}

pub fn update_status(conn: &Connection, name: &str, status: PackageStatus) -> Result<()> {
    conn.execute(
        "UPDATE packages SET status = ?1 WHERE name = ?2",
        params![status.as_str(), name],
    )
    .context("failed to update status")?;

    Ok(())
}

pub fn get_package(conn: &Connection, name: &str) -> Result<Option<Package>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, install_dir, shims, dependencies, status, installed_at
         FROM packages WHERE name = ?1",
    )?;

    let mut rows = stmt.query(params![name])?;

    if let Some(row) = rows.next()? {
        Ok(Some(row_to_package(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_packages(conn: &Connection) -> Result<Vec<Package>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, install_dir, shims, dependencies, status, installed_at
         FROM packages WHERE status = 'ok'
         ORDER BY name ASC",
    )?;

    let rows = stmt.query_map([], |row| Ok(row_to_package(row)))?;

    let mut packages = Vec::new();
    for row in rows {
        packages.push(row?.context("failed to read row")?);
    }

    Ok(packages)
}

pub fn delete_package(conn: &Connection, name: &str) -> Result<bool> {
    let affected = conn
        .execute("DELETE FROM packages WHERE name = ?1", params![name])
        .context("failed to delete package")?;

    Ok(affected > 0)
}

pub fn config_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .context("failed to set config")?;

    Ok(())
}

pub fn config_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM config WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;

    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

fn row_to_package(row: &rusqlite::Row) -> Result<Package> {
    let shims_raw: String = row.get("shims")?;
    let dependencies_raw: String = row.get("dependencies")?;
    let status_raw: String = row.get("status")?;

    let shims: Vec<Shim> =
        serde_json::from_str(&shims_raw).context("failed to parse shims")?;

    let dependencies: Vec<String> =
        serde_json::from_str(&dependencies_raw).context("failed to parse dependencies")?;

    Ok(Package {
        name: row.get("name")?,
        version: row.get("version")?,
        kind: row.get("kind")?,
        install_dir: row.get("install_dir")?,
        shims,
        dependencies,
        status: PackageStatus::from_str(&status_raw),
        installed_at: row.get("installed_at")?,
    })
}

pub fn now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
