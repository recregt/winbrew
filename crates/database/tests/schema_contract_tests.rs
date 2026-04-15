use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, params};
use tempfile::TempDir;
use winbrew_database as database;
use winbrew_database::Config;
use winbrew_models::catalog::CatalogInstallerType;
use winbrew_models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION;
use winbrew_models::domains::shared::DeploymentKind;

const CATALOG_SCHEMA: &str = include_str!("../../../infra/parser/schema/catalog.sql");

fn test_root() -> TempDir {
    tempfile::tempdir().expect("failed to create test root")
}

fn init_database(root: &Path) -> Result<()> {
    let config = Config::load_at(root)?;
    database::init(&config.resolved_paths())?;
    Ok(())
}

fn object_exists(conn: &Connection, object_type: &str, name: &str) -> Result<bool> {
    let exists = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = ?1 AND name = ?2)",
        params![object_type, name],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(exists != 0)
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    let pragma = format!("PRAGMA table_info('{table}')");
    let mut stmt = conn.prepare(&pragma)?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(columns.into_iter().collect())
}

fn column_notnull(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let pragma = format!("PRAGMA table_info('{table}')");
    let mut stmt = conn.prepare(&pragma)?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            let notnull: i64 = row.get(3)?;
            return Ok(notnull != 0);
        }
    }

    anyhow::bail!("column {column} not found in {table}");
}

fn column_default(conn: &Connection, table: &str, column: &str) -> Result<Option<String>> {
    let pragma = format!("PRAGMA table_info('{table}')");
    let mut stmt = conn.prepare(&pragma)?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(row.get::<_, Option<String>>(4)?);
        }
    }

    anyhow::bail!("column {column} not found in {table}");
}

fn installer_type_check_values(schema: &str) -> Result<Vec<String>> {
    let marker = "installer_type IN (";
    let start = schema
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| anyhow::anyhow!("installer_type CHECK clause not found"))?;
    let end = schema[start..]
        .find(")")
        .ok_or_else(|| anyhow::anyhow!("installer_type CHECK clause is not closed"))?;

    let values = schema[start..start + end]
        .split(',')
        .map(|value| value.trim().trim_matches('\'').to_string())
        .collect::<Vec<_>>();

    Ok(values)
}

fn model_installer_type_values() -> Vec<&'static str> {
    [
        CatalogInstallerType::Msi,
        CatalogInstallerType::Msix,
        CatalogInstallerType::Appx,
        CatalogInstallerType::Exe,
        CatalogInstallerType::Inno,
        CatalogInstallerType::Nullsoft,
        CatalogInstallerType::Wix,
        CatalogInstallerType::Burn,
        CatalogInstallerType::Pwa,
        CatalogInstallerType::Font,
        CatalogInstallerType::Portable,
        CatalogInstallerType::Zip,
        CatalogInstallerType::Nuget,
        CatalogInstallerType::Scoop,
        CatalogInstallerType::Unknown,
    ]
    .into_iter()
    .map(CatalogInstallerType::as_str)
    .collect()
}

fn insert_catalog_package(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO catalog_packages (
            id, name, version, source, namespace, source_id, description, homepage, license, publisher
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        params![
            "winget/Contoso.App",
            "Contoso App",
            "1.0.0",
            "winget",
            Option::<String>::None,
            "Contoso.App",
            Option::<String>::None,
            Option::<String>::None,
            Option::<String>::None,
            Option::<String>::None,
        ],
    )?;

    Ok(())
}

fn insert_catalog_installer(conn: &Connection, installer_type: &str) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO catalog_installers (
            package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind, nested_kind
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        params![
            "winget/Contoso.App",
            "https://example.test/app.exe",
            Option::<String>::None,
            "sha256",
            installer_type,
            Option::<String>::None,
            "x64",
            "exe",
            Option::<String>::None,
        ],
    )?;

    Ok(())
}

#[test]
fn catalog_contract_matches_canonical_schema() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(CATALOG_SCHEMA)?;

    assert_eq!(
        conn.query_row::<i64, _, _>("PRAGMA user_version", [], |row| row.get(0))?,
        i64::from(CATALOG_DB_SCHEMA_VERSION)
    );

    for (object_type, name) in [
        ("table", "schema_meta"),
        ("table", "catalog_packages"),
        ("table", "catalog_installers"),
        ("table", "catalog_packages_raw"),
        ("table", "catalog_packages_fts"),
        ("trigger", "catalog_packages_au"),
        ("trigger", "catalog_packages_update_timestamp"),
        ("index", "idx_catalog_installers_unique"),
    ] {
        assert!(
            object_exists(&conn, object_type, name)?,
            "expected {object_type} {name} to exist"
        );
    }

    let installer_columns = table_columns(&conn, "catalog_installers")?;
    assert!(installer_columns.contains("kind"));
    assert!(installer_columns.contains("scope"));
    assert!(!installer_columns.contains("type"));
    assert!(!column_notnull(&conn, "catalog_installers", "scope")?);
    assert_eq!(column_default(&conn, "catalog_installers", "scope")?, None);

    let schema_version: String = conn.query_row(
        "SELECT value FROM schema_meta WHERE name = 'schema_version'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(schema_version, CATALOG_DB_SCHEMA_VERSION.to_string());

    assert_eq!(
        installer_type_check_values(CATALOG_SCHEMA)?,
        model_installer_type_values()
    );

    assert!(!column_notnull(&conn, "catalog_installers", "hash")?);
    assert_eq!(column_default(&conn, "catalog_installers", "hash")?, None);

    insert_catalog_package(&conn)?;

    for installer_type in [
        "msi", "msix", "appx", "exe", "inno", "nullsoft", "wix", "burn", "pwa", "font", "portable",
        "zip", "nuget", "scoop",
    ] {
        insert_catalog_installer(&conn, installer_type)?;
    }

    assert!(insert_catalog_installer(&conn, "nsis").is_err());

    Ok(())
}

#[test]
fn main_database_contract_matches_database_schema() -> Result<()> {
    let test_root = test_root();
    init_database(test_root.path())?;

    let conn = database::get_conn()?;

    for table in [
        "installed_packages",
        "msi_receipts",
        "msi_files",
        "msi_registry_entries",
        "msi_shortcuts",
        "msi_components",
    ] {
        assert!(object_exists(&conn, "table", table)?);
    }

    let installed_columns = table_columns(&conn, "installed_packages")?;
    for column in [
        "name",
        "version",
        "kind",
        "deployment_kind",
        "engine_kind",
        "engine_metadata",
        "install_dir",
        "dependencies",
        "status",
        "installed_at",
    ] {
        assert!(
            installed_columns.contains(column),
            "missing column {column}"
        );
    }

    assert!(column_notnull(
        &conn,
        "installed_packages",
        "deployment_kind"
    )?);
    assert_eq!(
        column_default(&conn, "installed_packages", "deployment_kind")?.as_deref(),
        Some("'installed'")
    );

    Ok(())
}

#[test]
fn journal_contract_round_trips_metadata_and_commit() -> Result<()> {
    let test_root = test_root();

    let mut writer =
        database::JournalWriter::open_for_package(test_root.path(), "winget/Contoso.App", "1.0.0")?;

    writer.append(&database::JournalEntry::Metadata {
        package_id: "winget/Contoso.App".to_string(),
        version: "1.0.0".to_string(),
        engine: "portable".to_string(),
        deployment_kind: DeploymentKind::Portable,
        install_dir: r"C:\winbrew\apps\Contoso.App".to_string(),
        dependencies: vec!["winget/Contoso.Shared".to_string()],
        engine_metadata: None,
    })?;
    writer.append(&database::JournalEntry::Commit {
        installed_at: "2026-04-14T00:00:00Z".to_string(),
    })?;
    writer.flush()?;

    let journal_path = writer.path().to_path_buf();
    drop(writer);

    let contents = std::fs::read_to_string(&journal_path)?;
    assert!(contents.contains("\"deployment_kind\":\"portable\""));

    let entries = database::JournalReader::read_committed(&journal_path)?;
    assert_eq!(entries.len(), 2);

    match &entries[0] {
        database::JournalEntry::Metadata {
            package_id,
            version,
            engine,
            deployment_kind,
            install_dir,
            dependencies,
            engine_metadata,
        } => {
            assert_eq!(package_id, "winget/Contoso.App");
            assert_eq!(version, "1.0.0");
            assert_eq!(engine, "portable");
            assert_eq!(*deployment_kind, DeploymentKind::Portable);
            assert_eq!(install_dir, r"C:\winbrew\apps\Contoso.App");
            assert_eq!(dependencies, &vec!["winget/Contoso.Shared".to_string()]);
            assert!(engine_metadata.is_none());
        }
        other => panic!("expected metadata entry, got {other:?}"),
    }

    assert!(matches!(entries[1], database::JournalEntry::Commit { .. }));

    Ok(())
}
