use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, Error as SqlError, OptionalExtension, params, types::Type};

use crate::models::{Package, PackageStatus};

pub fn insert_package(conn: &Connection, pkg: &Package) -> Result<()> {
    let deps =
        serde_json::to_string(&pkg.dependencies).context("failed to serialize dependencies")?;

    conn.execute(
        "INSERT INTO installed_packages
         (name, version, kind, install_dir, product_code, dependencies, status, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            pkg.name,
            pkg.version,
            pkg.kind,
            pkg.install_dir,
            pkg.product_code,
            deps,
            pkg.status.as_str(),
            pkg.installed_at,
        ],
    )
    .context("failed to insert package")?;

    Ok(())
}

pub fn update_status(conn: &Connection, name: &str, status: PackageStatus) -> Result<()> {
    let affected = conn
        .execute(
            "UPDATE installed_packages SET status = ?1 WHERE name = ?2",
            params![status.as_str(), name],
        )
        .context("failed to update status")?;

    if affected == 0 {
        return Err(anyhow!("package '{name}' not found"));
    }

    Ok(())
}

pub fn get_package(conn: &Connection, name: &str) -> Result<Option<Package>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, install_dir, product_code, dependencies, status, installed_at
            FROM installed_packages WHERE name = ?1",
    )?;

    stmt.query_row(params![name], row_to_package)
        .optional()
        .context("failed to query package")
}

pub fn list_packages(conn: &Connection) -> Result<Vec<Package>> {
    let mut stmt = conn.prepare(
        // Returns only packages that completed successfully.
        "SELECT name, version, kind, install_dir, product_code, dependencies, status, installed_at
            FROM installed_packages WHERE status = 'ok'
         ORDER BY name ASC",
    )?;

    stmt.query_map([], |row| Ok(row_to_package(row)))?
        .map(|row| row?.context("failed to read row"))
        .collect()
}

pub fn delete_package(conn: &Connection, name: &str) -> Result<bool> {
    let affected = conn
        .execute(
            "DELETE FROM installed_packages WHERE name = ?1",
            params![name],
        )
        .context("failed to delete package")?;

    Ok(affected > 0)
}

fn row_to_package(row: &rusqlite::Row) -> std::result::Result<Package, SqlError> {
    const COL_DEPENDENCIES: usize = 5;

    let dependencies_raw: String = row.get("dependencies")?;
    let status_raw: String = row.get("status")?;

    let dependencies: Vec<String> = serde_json::from_str(&dependencies_raw).map_err(|err| {
        SqlError::FromSqlConversionFailure(COL_DEPENDENCIES, Type::Text, Box::new(err))
    })?;

    Ok(Package {
        name: row.get("name")?,
        version: row.get("version")?,
        kind: row.get("kind")?,
        install_dir: row.get("install_dir")?,
        product_code: row.get("product_code")?,
        dependencies,
        status: PackageStatus::parse(&status_raw),
        installed_at: row.get("installed_at")?,
    })
}
