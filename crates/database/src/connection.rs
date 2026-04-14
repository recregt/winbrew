use anyhow::{Context, Result};
use r2d2::{ManageConnection, Pool};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SqliteConnectionManager {
    path: PathBuf,
    read_only: bool,
}

impl ManageConnection for SqliteConnectionManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> std::result::Result<Self::Connection, Self::Error> {
        open_connection(&self.path, self.read_only)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> std::result::Result<(), Self::Error> {
        conn.execute_batch("SELECT 1;")
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

pub(crate) fn open_connection(
    path: &Path,
    read_only: bool,
) -> std::result::Result<Connection, rusqlite::Error> {
    let conn = if read_only {
        if !path.exists() {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                Some(format!("catalog database not found: {}", path.display())),
            ));
        }

        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?
    } else {
        Connection::open(path)?
    };
    conn.busy_timeout(Duration::from_secs(5))?;

    if read_only {
        conn.execute_batch("PRAGMA query_only=ON; PRAGMA foreign_keys=ON;")?;
    } else {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )?;
    }

    Ok(conn)
}

pub(crate) fn build_pool(
    path: PathBuf,
    read_only: bool,
    max_size: u32,
    migrate: Option<fn(&Connection) -> Result<()>>,
) -> Result<Pool<SqliteConnectionManager>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create winbrew database directory")?;
    }

    let pool = Pool::builder()
        .max_size(max_size)
        .build(SqliteConnectionManager { path, read_only })
        .context("failed to initialize SQLite connection pool")?;

    if let Some(migrate) = migrate {
        let conn = pool
            .get()
            .context("failed to get database connection for migrations")?;
        migrate(&conn)?;
    }

    Ok(pool)
}
