use anyhow::{Context, Result};
use rusqlite::{Connection, Error as SqlError, OptionalExtension, params, types::Type};
use thiserror::Error;

use winbrew_models::{InstallerType, Package, PackageStatus};

#[derive(Debug, Error)]
#[error("package '{name}' not found")]
pub struct PackageNotFoundError {
    pub name: String,
}

pub fn insert_package(conn: &Connection, pkg: &Package) -> Result<()> {
    let deps =
        serde_json::to_string(&pkg.dependencies).context("failed to serialize dependencies")?;

    conn.execute(
        "INSERT INTO installed_packages
         (name, version, kind, install_dir, msix_package_full_name, dependencies, status, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            pkg.name,
            pkg.version,
            pkg.kind.to_string(),
            pkg.install_dir,
            pkg.msix_package_full_name,
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
        return Err(PackageNotFoundError {
            name: name.to_string(),
        }
        .into());
    }

    Ok(())
}

pub fn update_status_and_msix_package_full_name(
    conn: &Connection,
    name: &str,
    status: PackageStatus,
    msix_package_full_name: Option<&str>,
) -> Result<()> {
    let affected = conn
        .execute(
            "UPDATE installed_packages
                SET status = ?1,
                    msix_package_full_name = ?2
              WHERE name = ?3",
            params![status.as_str(), msix_package_full_name, name],
        )
        .context("failed to update status and msix package full name")?;

    if affected == 0 {
        return Err(PackageNotFoundError {
            name: name.to_string(),
        }
        .into());
    }

    Ok(())
}

pub fn get_package(conn: &Connection, name: &str) -> Result<Option<Package>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, install_dir, msix_package_full_name, dependencies, status, installed_at
            FROM installed_packages WHERE name = ?1",
    )?;

    stmt.query_row(params![name], row_to_package)
        .optional()
        .context("failed to query package")
}

pub fn list_packages(conn: &Connection) -> Result<Vec<Package>> {
    let mut stmt = conn.prepare(
        // Returns only packages that completed successfully.
        "SELECT name, version, kind, install_dir, msix_package_full_name, dependencies, status, installed_at
            FROM installed_packages WHERE status = 'ok'
         ORDER BY name ASC",
    )?;

    stmt.query_map([], |row| Ok(row_to_package(row)))?
        .map(|row| row?.context("failed to read row"))
        .collect()
}

pub fn list_installing_packages(conn: &Connection) -> Result<Vec<Package>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, install_dir, msix_package_full_name, dependencies, status, installed_at
            FROM installed_packages WHERE status = 'installing'
         ORDER BY installed_at ASC, name ASC",
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
    const COL_KIND: usize = 2;
    const COL_DEPENDENCIES: usize = 4;

    let dependencies_raw: String = row.get("dependencies")?;
    let status_raw: String = row.get("status")?;
    let kind_raw: String = row.get("kind")?;

    let dependencies: Vec<String> = serde_json::from_str(&dependencies_raw).map_err(|err| {
        SqlError::FromSqlConversionFailure(COL_DEPENDENCIES, Type::Text, Box::new(err))
    })?;
    let kind = kind_raw
        .parse::<InstallerType>()
        .map_err(|err| SqlError::FromSqlConversionFailure(COL_KIND, Type::Text, Box::new(err)))?;

    Ok(Package {
        name: row.get("name")?,
        version: row.get("version")?,
        kind,
        install_dir: row.get("install_dir")?,
        msix_package_full_name: row.get("msix_package_full_name")?,
        dependencies,
        status: PackageStatus::parse(&status_raw),
        installed_at: row.get("installed_at")?,
    })
}
