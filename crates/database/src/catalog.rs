use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use super::CatalogSchemaVersionMismatchError;
use winbrew_models::catalog::installer_type::CatalogInstallerType;
use winbrew_models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION as CATALOG_SCHEMA_VERSION;
use winbrew_models::catalog::package::{CatalogInstaller, CatalogPackage};
use winbrew_models::catalog::raw::RawCatalogInstaller;
use winbrew_models::shared::HashAlgorithm;

pub fn search(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.version, p.source, p.namespace, p.source_id, p.created_at, p.updated_at, p.description, p.homepage, p.license, p.publisher, p.locale, p.moniker, p.platform, p.commands, p.protocols, p.file_extensions, p.capabilities, p.tags, p.bin
         FROM catalog_packages p
         JOIN catalog_packages_fts fts ON p.rowid = fts.rowid
         WHERE catalog_packages_fts MATCH ?1
         ORDER BY bm25(catalog_packages_fts), p.name ASC",
    )?;

    stmt.query_map(params![query], row_to_package)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read catalog package")
}

pub fn get_installers(conn: &Connection, package_id: &str) -> Result<Vec<CatalogInstaller>> {
    let mut stmt = conn.prepare(
        "SELECT package_id, url, hash, hash_algorithm, installer_type, installer_switches, platform, commands, protocols, file_extensions, capabilities, scope, arch, kind, nested_kind
         FROM catalog_installers
         WHERE package_id = ?1
         ORDER BY scope ASC, arch ASC, kind ASC, installer_type ASC, nested_kind ASC, installer_switches ASC, hash_algorithm ASC, url ASC",
    )?;

    stmt.query_map(params![package_id], row_to_installer)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read catalog installer")
}

pub fn get_package_by_id(conn: &Connection, package_id: &str) -> Result<Option<CatalogPackage>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, source, namespace, source_id, created_at, updated_at, description, homepage, license, publisher, locale, moniker, platform, commands, protocols, file_extensions, capabilities, tags, bin
         FROM catalog_packages
         WHERE id = ?1",
    )?;

    stmt.query_row(params![package_id], row_to_package)
        .optional()
        .context("failed to read catalog package")
}

pub fn ensure_schema_version(conn: &Connection) -> Result<()> {
    let schema_meta_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_meta')",
        [],
        |row| row.get::<_, i64>(0),
    )? != 0;

    let actual_version: i64 = if schema_meta_exists {
        let version_text: String = conn.query_row(
            "SELECT value FROM schema_meta WHERE name = 'schema_version'",
            [],
            |row| row.get(0),
        )?;
        version_text.parse().with_context(|| {
            format!("failed to parse schema_meta schema_version value: {version_text}")
        })?
    } else {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))?
    };
    let expected_version = i64::from(CATALOG_SCHEMA_VERSION);

    if actual_version != expected_version {
        return Err(CatalogSchemaVersionMismatchError {
            expected: CATALOG_SCHEMA_VERSION,
            actual: actual_version,
        }
        .into());
    }

    Ok(())
}

fn row_to_package(row: &rusqlite::Row) -> rusqlite::Result<CatalogPackage> {
    let version = row.get::<_, String>("version")?.parse().map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let source = row.get::<_, String>("source")?.parse().map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;

    let package = CatalogPackage {
        id: row.get::<_, String>("id")?.into(),
        name: row.get("name")?,
        version,
        source,
        namespace: row.get("namespace")?,
        source_id: row.get("source_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        description: row.get("description")?,
        homepage: row.get("homepage")?,
        license: row.get("license")?,
        publisher: row.get("publisher")?,
        locale: row.get("locale")?,
        moniker: row.get("moniker")?,
        platform: row.get("platform")?,
        commands: row.get("commands")?,
        protocols: row.get("protocols")?,
        file_extensions: row.get("file_extensions")?,
        capabilities: row.get("capabilities")?,
        tags: row.get("tags")?,
        bin: row.get("bin")?,
    };

    package.validate().map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(package)
}

fn row_to_installer(row: &rusqlite::Row) -> rusqlite::Result<CatalogInstaller> {
    let hash = row.get::<_, Option<String>>("hash")?.unwrap_or_default();

    let raw = RawCatalogInstaller {
        package_id: row.get::<_, String>("package_id")?,
        url: row.get("url")?,
        hash,
        hash_algorithm: row
            .get::<_, String>("hash_algorithm")?
            .parse::<HashAlgorithm>()
            .map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?,
        installer_type: row
            .get::<_, String>("installer_type")?
            .parse::<CatalogInstallerType>()
            .map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?,
        installer_switches: row.get("installer_switches")?,
        platform: row.get("platform")?,
        commands: row.get("commands")?,
        protocols: row.get("protocols")?,
        file_extensions: row.get("file_extensions")?,
        capabilities: row.get("capabilities")?,
        scope: row.get("scope")?,
        arch: row.get("arch")?,
        kind: row.get("kind")?,
        nested_kind: row.get("nested_kind")?,
    };

    CatalogInstaller::try_from(raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CATALOG_SCHEMA_VERSION, ensure_schema_version, get_installers, get_package_by_id, search,
    };
    use rusqlite::Connection;
    use winbrew_models::catalog::CatalogInstallerType;
    use winbrew_models::install::installer::InstallerType;
    use winbrew_models::shared::HashAlgorithm;

    const CATALOG_SCHEMA: &str = include_str!("../../../infra/parser/schema/catalog.sql");

    fn create_catalog_installers_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE catalog_installers (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    package_id TEXT NOT NULL,\n    url TEXT NOT NULL,\n    hash TEXT,\n    hash_algorithm TEXT NOT NULL DEFAULT 'sha256',\n    installer_type TEXT NOT NULL DEFAULT 'unknown',\n    installer_switches TEXT,\n    platform TEXT,\n    commands TEXT,\n    protocols TEXT,\n    file_extensions TEXT,\n    capabilities TEXT,\n    scope TEXT,\n    arch TEXT NOT NULL DEFAULT '',\n    kind TEXT NOT NULL DEFAULT '',\n    nested_kind TEXT\n);",
        )
        .expect("catalog installers table should be created");
    }

    #[test]
    fn get_installers_reads_nested_kind_when_present() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        create_catalog_installers_table(&conn);

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "https://example.test/app-one.zip",
                "sha256:deadbeef",
                "sha256",
                "zip",
                Option::<String>::None,
                "x64",
                "zip",
                "portable",
            ],
        )
        .expect("insert portable installer");

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "https://example.test/app-two.zip",
                "sha256:deadbeef",
                "sha256",
                "zip",
                Option::<String>::None,
                "x64",
                "zip",
                "msi",
            ],
        )
        .expect("insert msi installer");

        let installers = get_installers(&conn, "winget/Contoso.App")
            .expect("catalog installers should load with nested kind");

        assert_eq!(installers.len(), 2);
        assert_eq!(installers[0].nested_kind, Some(InstallerType::Msi));
        assert_eq!(installers[1].nested_kind, Some(InstallerType::Portable));
        assert_eq!(installers[0].hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(installers[1].hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(installers[0].installer_type, CatalogInstallerType::Zip);
        assert_eq!(installers[1].installer_type, CatalogInstallerType::Zip);
    }

    #[test]
    fn get_installers_reads_null_hash_as_empty_string() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        create_catalog_installers_table(&conn);

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "https://example.test/app.exe",
                Option::<String>::None,
                "sha256",
                "exe",
                Option::<String>::None,
                "x64",
                "exe",
                Option::<String>::None,
            ],
        )
        .expect("insert checksumless installer");

        let installers = get_installers(&conn, "winget/Contoso.App")
            .expect("catalog installers should load with null hash");

        assert_eq!(installers.len(), 1);
        assert!(installers[0].hash.is_empty());
        assert_eq!(installers[0].kind, InstallerType::Exe);
    }

    #[test]
    fn package_queries_read_timestamps() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(CATALOG_SCHEMA)
            .expect("catalog schema should load");

        conn.execute(
            r#"
            INSERT INTO catalog_packages (
                id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "Contoso App",
                "1.2.3",
                "winget",
                Option::<String>::None,
                "Contoso.App",
                Some("Example package"),
                Option::<String>::None,
                Option::<String>::None,
                Some("Contoso Ltd."),
                Some("en-US"),
                "2026-04-14 12:00:00",
                "2026-04-14 12:34:56",
            ],
        )
        .expect("insert catalog package");

        let package = get_package_by_id(&conn, "winget/Contoso.App")
            .expect("package lookup should succeed")
            .expect("package should exist");
        let searched = search(&conn, "Contoso").expect("catalog search should succeed");

        assert_eq!(package.created_at.as_deref(), Some("2026-04-14 12:00:00"));
        assert_eq!(package.updated_at.as_deref(), Some("2026-04-14 12:34:56"));
        assert_eq!(searched.len(), 1);
        assert_eq!(
            searched[0].created_at.as_deref(),
            Some("2026-04-14 12:00:00")
        );
        assert_eq!(
            searched[0].updated_at.as_deref(),
            Some("2026-04-14 12:34:56")
        );
    }

    #[test]
    fn package_updates_refresh_updated_at_automatically() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(CATALOG_SCHEMA)
            .expect("catalog schema should load");

        conn.execute(
            r#"
            INSERT INTO catalog_packages (
                id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "Contoso App",
                "1.2.3",
                "winget",
                Option::<String>::None,
                "Contoso.App",
                Some("Example package"),
                Option::<String>::None,
                Option::<String>::None,
                Some("Contoso Ltd."),
                Some("en-US"),
                "2026-04-14 12:00:00",
                "2026-04-14 12:34:56",
            ],
        )
        .expect("insert catalog package");

        conn.execute(
            r#"
            UPDATE catalog_packages
            SET description = ?1
            WHERE id = ?2
            "#,
            rusqlite::params!["Updated package", "winget/Contoso.App"],
        )
        .expect("update catalog package");

        let package = get_package_by_id(&conn, "winget/Contoso.App")
            .expect("package lookup should succeed")
            .expect("package should exist");

        assert_eq!(package.description.as_deref(), Some("Updated package"));
        assert_ne!(package.updated_at.as_deref(), Some("2026-04-14 12:34:56"));
        assert_eq!(package.created_at.as_deref(), Some("2026-04-14 12:00:00"));
    }

    #[test]
    fn ensure_schema_version_accepts_expected_version() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(&format!(
            "CREATE TABLE schema_meta (name TEXT PRIMARY KEY, value TEXT NOT NULL);\nINSERT INTO schema_meta (name, value) VALUES ('schema_version', '{}');",
            CATALOG_SCHEMA_VERSION
        ))
        .expect("set schema version");

        ensure_schema_version(&conn).expect("schema version should match");
    }

    #[test]
    fn ensure_schema_version_rejects_mismatch() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        let err = ensure_schema_version(&conn).expect_err("schema mismatch should fail");

        assert!(
            err.to_string()
                .contains("Package catalog schema version mismatch")
        );
    }
}
