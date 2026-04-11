use anyhow::{Context, Result};
use rusqlite::{Connection, Error as SqlError, OptionalExtension, params, types::Type};
use thiserror::Error;

use winbrew_models::{EngineKind, EngineMetadata, InstallerType, Package, PackageStatus};

#[derive(Debug, Error)]
#[error("package '{name}' not found")]
pub struct PackageNotFoundError {
    pub name: String,
}

pub fn insert_package(conn: &Connection, pkg: &Package) -> Result<()> {
    let deps =
        serde_json::to_string(&pkg.dependencies).context("failed to serialize dependencies")?;
    let engine_metadata = pkg
        .engine_metadata
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("failed to serialize engine metadata")?;

    conn.execute(
        "INSERT INTO installed_packages
         (name, version, kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            pkg.name,
            pkg.version,
            pkg.kind.to_string(),
            pkg.engine_kind.to_string(),
            engine_metadata,
            pkg.install_dir,
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

pub fn update_status_and_engine_metadata(
    conn: &Connection,
    name: &str,
    status: PackageStatus,
    engine_metadata: Option<&EngineMetadata>,
    installed_at: &str,
) -> Result<()> {
    let engine_metadata = engine_metadata
        .map(serde_json::to_string)
        .transpose()
        .context("failed to serialize engine metadata")?;

    let affected = conn
        .execute(
            "UPDATE installed_packages
                SET status = ?1,
                    engine_metadata = ?2,
                    installed_at = ?3
              WHERE name = ?4",
            params![status.as_str(), engine_metadata, installed_at, name],
        )
        .context("failed to update status and engine metadata")?;

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
        "SELECT name, version, kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at
            FROM installed_packages WHERE name = ?1",
    )?;

    stmt.query_row(params![name], row_to_package)
        .optional()
        .context("failed to query package")
}

pub fn list_packages(conn: &Connection) -> Result<Vec<Package>> {
    let mut stmt = conn.prepare(
        // Returns only packages that completed successfully.
        "SELECT name, version, kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at
            FROM installed_packages WHERE status = 'ok'
         ORDER BY name ASC",
    )?;

    stmt.query_map([], |row| Ok(row_to_package(row)))?
        .map(|row| row?.context("failed to read row"))
        .collect()
}

pub fn list_installing_packages(conn: &Connection) -> Result<Vec<Package>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at
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
    const COL_ENGINE_KIND: usize = 3;
    const COL_ENGINE_METADATA: usize = 4;
    const COL_DEPENDENCIES: usize = 6;
    const COL_STATUS: usize = 7;

    let dependencies_raw: String = row.get("dependencies")?;
    let status_raw: String = row.get("status")?;
    let kind_raw: String = row.get("kind")?;
    let engine_kind_raw: String = row.get("engine_kind")?;
    let engine_metadata_raw: Option<String> = row.get("engine_metadata")?;

    let dependencies: Vec<String> = serde_json::from_str(&dependencies_raw).map_err(|err| {
        SqlError::FromSqlConversionFailure(COL_DEPENDENCIES, Type::Text, Box::new(err))
    })?;
    let kind = kind_raw
        .parse::<InstallerType>()
        .map_err(|err| SqlError::FromSqlConversionFailure(COL_KIND, Type::Text, Box::new(err)))?;
    let engine_kind = engine_kind_raw.parse::<EngineKind>().map_err(|err| {
        SqlError::FromSqlConversionFailure(COL_ENGINE_KIND, Type::Text, Box::new(err))
    })?;
    let engine_metadata = match engine_metadata_raw {
        Some(value) => Some(serde_json::from_str(&value).map_err(|err| {
            SqlError::FromSqlConversionFailure(COL_ENGINE_METADATA, Type::Text, Box::new(err))
        })?),
        None => None,
    };
    let status = status_raw
        .parse::<PackageStatus>()
        .map_err(|err| SqlError::FromSqlConversionFailure(COL_STATUS, Type::Text, Box::new(err)))?;

    Ok(Package {
        name: row.get("name")?,
        version: row.get("version")?,
        kind,
        engine_kind,
        engine_metadata,
        install_dir: row.get("install_dir")?,
        dependencies,
        status,
        installed_at: row.get("installed_at")?,
    })
}
