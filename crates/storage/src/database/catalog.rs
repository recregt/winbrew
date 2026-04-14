use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use super::CatalogSchemaVersionMismatchError;
use winbrew_models::catalog::metadata::SCHEMA_VERSION as CATALOG_SCHEMA_VERSION;
use winbrew_models::catalog::package::{CatalogInstaller, CatalogPackage};
use winbrew_models::catalog::raw::{RawCatalogInstaller, RawCatalogPackage};

pub fn search(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.version, p.description, p.homepage, p.license, p.publisher
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
    let query = if catalog_installers_has_nested_kind(conn)? {
        "SELECT package_id, url, hash, arch, type, nested_kind
         FROM catalog_installers
         WHERE package_id = ?1
         ORDER BY arch ASC, type ASC, nested_kind ASC, url ASC"
    } else {
        "SELECT package_id, url, hash, arch, type, NULL AS nested_kind
         FROM catalog_installers
         WHERE package_id = ?1
         ORDER BY arch ASC, type ASC, url ASC"
    };

    let mut stmt = conn.prepare(query)?;

    stmt.query_map(params![package_id], row_to_installer)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read catalog installer")
}

fn catalog_installers_has_nested_kind(conn: &Connection) -> Result<bool> {
    let mut stmt = conn.prepare("PRAGMA table_info(catalog_installers)")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let column_name: String = row.get(1)?;
        if column_name == "nested_kind" {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn get_package_by_id(conn: &Connection, package_id: &str) -> Result<Option<CatalogPackage>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, description, homepage, license, publisher
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
        source: None,
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
    use super::{ensure_schema_version, get_installers};
    use rusqlite::Connection;
    use winbrew_models::install::installer::InstallerType;

    fn create_catalog_installers_table(conn: &Connection, with_nested_kind: bool) {
        let nested_kind_column = if with_nested_kind {
            ",\n    nested_kind TEXT"
        } else {
            ""
        };

        let schema = format!(
            "CREATE TABLE catalog_installers (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    package_id TEXT NOT NULL,\n    url TEXT NOT NULL,\n    hash TEXT NOT NULL,\n    arch TEXT NOT NULL DEFAULT '',\n    type TEXT NOT NULL DEFAULT ''{nested_kind_column}\n);"
        );

        conn.execute_batch(&schema)
            .expect("catalog installers table should be created");
    }

    #[test]
    fn get_installers_reads_legacy_schema_without_nested_kind() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        create_catalog_installers_table(&conn, false);

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, arch, type)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            rusqlite::params![
                "winget/Contoso.App",
                "https://example.test/app.exe",
                "sha256:deadbeef",
                "x64",
                "zip",
            ],
        )
        .expect("insert legacy installer");

        let installers = get_installers(&conn, "winget/Contoso.App")
            .expect("legacy catalog installers should load");

        assert_eq!(installers.len(), 1);
        assert_eq!(installers[0].kind, InstallerType::Zip);
        assert_eq!(installers[0].nested_kind, None);
    }

    #[test]
    fn get_installers_reads_nested_kind_when_present() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        create_catalog_installers_table(&conn, true);

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
        conn.execute("PRAGMA user_version = 1", [])
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
