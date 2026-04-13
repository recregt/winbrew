use anyhow::{Context, Result};
use rusqlite::{Connection, Error as SqlError, OptionalExtension, params, types::Type};
use thiserror::Error;

use crate::core::now;
use winbrew_models::install::engine::{EngineInstallReceipt, EngineKind, EngineMetadata};
use winbrew_models::install::installed::{InstalledPackage, PackageStatus};
use winbrew_models::install::installer::InstallerType;
use winbrew_models::shared::DeploymentKind;

#[derive(Debug, Error)]
#[error("package '{name}' not found")]
pub struct PackageNotFoundError {
    pub name: String,
}

pub fn insert_package(conn: &Connection, pkg: &InstalledPackage) -> Result<()> {
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
         (name, version, kind, deployment_kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            pkg.name,
            pkg.version,
            pkg.kind.to_string(),
            pkg.deployment_kind.to_string(),
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
    install_dir: &str,
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
                                        install_dir = ?3,
                                        installed_at = ?4
                            WHERE name = ?5",
            params![
                status.as_str(),
                engine_metadata,
                install_dir,
                installed_at,
                name
            ],
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

pub fn commit_install(
    conn: &mut crate::database::DbConnection,
    name: &str,
    engine_receipt: &EngineInstallReceipt,
) -> Result<()> {
    let installed_at = now();
    let tx = conn
        .transaction()
        .context("failed to start install commit transaction")?;

    update_status_and_engine_metadata(
        &tx,
        name,
        PackageStatus::Ok,
        engine_receipt.engine_metadata.as_ref(),
        engine_receipt.install_dir.as_str(),
        &installed_at,
    )?;

    if let Some(snapshot) = engine_receipt.msi_inventory_snapshot.as_ref() {
        crate::database::apply_snapshot(&tx, snapshot)?;
    }

    tx.commit().context("failed to commit install state")?;

    Ok(())
}

pub fn replay_committed_journal(
    conn: &mut Connection,
    journal: &crate::database::CommittedJournalPackage,
) -> Result<()> {
    let tx = conn
        .transaction()
        .context("failed to start journal replay transaction")?;

    let _ = delete_package(&tx, &journal.package.name)?;
    insert_package(&tx, &journal.package)?;

    tx.commit()
        .context("failed to commit journal replay transaction")?;

    Ok(())
}

pub fn get_package(conn: &Connection, name: &str) -> Result<Option<InstalledPackage>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, deployment_kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at
            FROM installed_packages WHERE name = ?1",
    )?;

    stmt.query_row(params![name], row_to_package)
        .optional()
        .context("failed to query package")
}

pub fn list_packages(conn: &Connection) -> Result<Vec<InstalledPackage>> {
    let mut stmt = conn.prepare(
        // Returns only packages that completed successfully.
        "SELECT name, version, kind, deployment_kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at
            FROM installed_packages WHERE status = 'ok'
         ORDER BY name ASC",
    )?;

    stmt.query_map([], row_to_package)?
        .map(|row: rusqlite::Result<InstalledPackage>| row.context("failed to read row"))
        .collect()
}

pub fn list_installing_packages(conn: &Connection) -> Result<Vec<InstalledPackage>> {
    let mut stmt = conn.prepare(
        "SELECT name, version, kind, deployment_kind, engine_kind, engine_metadata, install_dir, dependencies, status, installed_at
            FROM installed_packages WHERE status = 'installing'
         ORDER BY installed_at ASC, name ASC",
    )?;

    stmt.query_map([], row_to_package)?
        .map(|row: rusqlite::Result<InstalledPackage>| row.context("failed to read row"))
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

fn row_to_package(row: &rusqlite::Row) -> std::result::Result<InstalledPackage, SqlError> {
    const COL_KIND: usize = 2;
    const COL_DEPLOYMENT_KIND: usize = 3;
    const COL_ENGINE_KIND: usize = 4;
    const COL_ENGINE_METADATA: usize = 5;
    const COL_DEPENDENCIES: usize = 7;
    const COL_STATUS: usize = 8;

    let dependencies_raw: String = row.get("dependencies")?;
    let status_raw: String = row.get("status")?;
    let kind_raw: String = row.get("kind")?;
    let deployment_kind_raw: String = row.get("deployment_kind")?;
    let engine_kind_raw: String = row.get("engine_kind")?;
    let engine_metadata_raw: Option<String> = row.get("engine_metadata")?;

    let dependencies: Vec<String> = serde_json::from_str(&dependencies_raw).map_err(|err| {
        SqlError::FromSqlConversionFailure(COL_DEPENDENCIES, Type::Text, Box::new(err))
    })?;
    let kind = kind_raw
        .parse::<InstallerType>()
        .map_err(|err| SqlError::FromSqlConversionFailure(COL_KIND, Type::Text, Box::new(err)))?;
    let deployment_kind = deployment_kind_raw
        .parse::<DeploymentKind>()
        .map_err(|err| {
            SqlError::FromSqlConversionFailure(COL_DEPLOYMENT_KIND, Type::Text, Box::new(err))
        })?;
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

    Ok(InstalledPackage {
        name: row.get("name")?,
        version: row.get("version")?,
        kind,
        deployment_kind,
        engine_kind,
        engine_metadata,
        install_dir: row.get("install_dir")?,
        dependencies,
        status,
        installed_at: row.get("installed_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        get_package, insert_package, replay_committed_journal, update_status_and_engine_metadata,
    };
    use crate::database::migration;
    use rusqlite::Connection;
    use std::path::PathBuf;
    use winbrew_models::install::engine::{EngineKind, EngineMetadata, InstallScope};
    use winbrew_models::install::installed::{InstalledPackage, PackageStatus};
    use winbrew_models::install::installer::InstallerType;
    use winbrew_models::shared::DeploymentKind;

    fn sample_package(name: &str) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Msi,
            deployment_kind: DeploymentKind::Installed,
            engine_kind: EngineKind::Msi,
            engine_metadata: Some(EngineMetadata::Msi {
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: None,
                scope: InstallScope::Installed,
                registry_keys: Vec::new(),
                shortcuts: Vec::new(),
            }),
            install_dir: "C:/Tools/Old".to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Installing,
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn update_status_and_engine_metadata_overwrites_install_dir() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        migration::migrate(&conn).expect("run migration");

        let package_name = "demo";
        insert_package(&conn, &sample_package(package_name)).expect("insert package");

        update_status_and_engine_metadata(
            &conn,
            package_name,
            PackageStatus::Ok,
            Some(&EngineMetadata::Msi {
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: Some("{22222222-2222-2222-2222-222222222222}".to_string()),
                scope: InstallScope::Installed,
                registry_keys: vec!["HKLM\\Software\\Demo".to_string()],
                shortcuts: vec!["C:/Users/Public/Desktop/Demo.lnk".to_string()],
            }),
            "C:/Tools/Actual",
            "2026-04-12T00:10:00Z",
        )
        .expect("update package state");

        let package = get_package(&conn, package_name)
            .expect("read updated package")
            .expect("package should exist");

        assert_eq!(package.install_dir, "C:/Tools/Actual");
        assert_eq!(package.status, PackageStatus::Ok);
        assert_eq!(
            package.engine_metadata.unwrap(),
            EngineMetadata::Msi {
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: Some("{22222222-2222-2222-2222-222222222222}".to_string()),
                scope: InstallScope::Installed,
                registry_keys: vec!["HKLM\\Software\\Demo".to_string()],
                shortcuts: vec!["C:/Users/Public/Desktop/Demo.lnk".to_string()],
            }
        );
    }

    #[test]
    fn replay_committed_journal_replaces_existing_package() {
        let mut conn = Connection::open_in_memory().expect("open in-memory database");
        migration::migrate(&conn).expect("run migration");

        let package_name = "demo";
        insert_package(&conn, &sample_package(package_name)).expect("insert original package");

        let replay_package = InstalledPackage {
            install_dir: "C:/Tools/Replayed".to_string(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-12T01:00:00Z".to_string(),
            ..sample_package(package_name)
        };

        let replay = crate::database::CommittedJournalPackage {
            journal_path: PathBuf::from("C:/tmp/journal.jsonl"),
            entries: Vec::new(),
            package: replay_package,
        };

        replay_committed_journal(&mut conn, &replay).expect("replay committed journal");

        let package = get_package(&conn, package_name)
            .expect("read replayed package")
            .expect("package should exist");

        assert_eq!(package.install_dir, "C:/Tools/Replayed");
        assert_eq!(package.status, PackageStatus::Ok);
        assert_eq!(package.installed_at, "2026-04-12T01:00:00Z");
    }
}
