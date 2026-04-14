use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use super::CatalogSchemaVersionMismatchError;
use winbrew_models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION as CATALOG_SCHEMA_VERSION;
use winbrew_models::catalog::package::{CatalogInstaller, CatalogPackage};
use winbrew_models::catalog::raw::{RawCatalogInstaller, RawCatalogPackage};

pub fn search(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.version, p.source, p.namespace, p.source_id, p.description, p.homepage, p.license, p.publisher
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
        "SELECT package_id, url, hash, arch, type, nested_kind
         FROM catalog_installers
         WHERE package_id = ?1
         ORDER BY arch ASC, type ASC, nested_kind ASC, url ASC",
    )?;

    stmt.query_map(params![package_id], row_to_installer)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read catalog installer")
}

pub fn get_package_by_id(conn: &Connection, package_id: &str) -> Result<Option<CatalogPackage>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, source, namespace, source_id, description, homepage, license, publisher
         FROM catalog_packages
         WHERE id = ?1",
    )?;

    stmt.query_row(params![package_id], row_to_package)
        .optional()
        .context("failed to read catalog package")
}

pub fn ensure_schema_version(conn: &Connection) -> Result<()> {
    let actual_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
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
    let raw = RawCatalogPackage {
        id: row.get::<_, String>("id")?,
        name: row.get("name")?,
        version: row.get("version")?,
        source: row.get("source")?,
        namespace: row.get("namespace")?,
        source_id: row.get("source_id")?,
        description: row.get("description")?,
        homepage: row.get("homepage")?,
        license: row.get("license")?,
        publisher: row.get("publisher")?,
    };

    CatalogPackage::try_from(raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

fn row_to_installer(row: &rusqlite::Row) -> rusqlite::Result<CatalogInstaller> {
    let raw = RawCatalogInstaller {
        package_id: row.get::<_, String>("package_id")?,
        url: row.get("url")?,
        hash: row.get("hash")?,
        arch: row.get("arch")?,
        kind: row.get("type")?,
        nested_kind: row.get("nested_kind")?,
    };

    CatalogInstaller::try_from(raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

#[cfg(test)]
mod tests {
    use super::{CATALOG_SCHEMA_VERSION, ensure_schema_version, get_installers};
    use rusqlite::Connection;
    use winbrew_models::install::installer::InstallerType;

    fn create_catalog_installers_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE catalog_installers (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    package_id TEXT NOT NULL,\n    url TEXT NOT NULL,\n    hash TEXT NOT NULL,\n    arch TEXT NOT NULL DEFAULT '',\n    type TEXT NOT NULL DEFAULT '',\n    nested_kind TEXT\n);",
        )
        .expect("catalog installers table should be created");
    }

    #[test]
    fn get_installers_reads_nested_kind_when_present() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        create_catalog_installers_table(&conn);

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, arch, type, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "https://example.test/app-one.zip",
                "sha256:deadbeef",
                "x64",
                "zip",
                "portable",
            ],
        )
        .expect("insert portable installer");

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, arch, type, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "https://example.test/app-two.zip",
                "sha256:deadbeef",
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
    }

    #[test]
    fn ensure_schema_version_accepts_expected_version() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute(
            &format!("PRAGMA user_version = {}", CATALOG_SCHEMA_VERSION),
            [],
        )
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
