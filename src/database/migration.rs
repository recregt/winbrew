use anyhow::{Context, Result};
use rusqlite::Connection;

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS installed_packages (
            name         TEXT PRIMARY KEY,
            version      TEXT NOT NULL,
            kind         TEXT NOT NULL,
            install_dir  TEXT NOT NULL,
            msix_package_full_name TEXT,
            dependencies TEXT NOT NULL DEFAULT '[]',
            status       TEXT NOT NULL DEFAULT 'installing',
            installed_at TEXT NOT NULL
        );
    ",
    )
    .context("migration failed")?;

    if !installed_packages_has_column(conn, "msix_package_full_name")? {
        conn.execute_batch(
            "ALTER TABLE installed_packages ADD COLUMN msix_package_full_name TEXT;",
        )
        .context("failed to add msix_package_full_name column")?;
    }

    Ok(())
}

fn installed_packages_has_column(conn: &Connection, column_name: &str) -> Result<bool> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(installed_packages)")
        .context("failed to inspect installed_packages schema")?;

    let mut rows = stmt
        .query([])
        .context("failed to query installed_packages schema")?;

    while let Some(row) = rows
        .next()
        .context("failed to read installed_packages schema")?
    {
        let name: String = row.get("name").context("failed to read column name")?;

        if name == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}
