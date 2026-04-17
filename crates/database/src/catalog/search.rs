use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use winbrew_models::catalog::package::CatalogPackage;

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

#[cfg(test)]
mod tests {
    use super::{get_package_by_id, search};
    use rusqlite::{Connection, params};

    const CATALOG_SCHEMA: &str = include_str!("../../../../infra/parser/schema/catalog.sql");

    fn insert_catalog_package(conn: &Connection) {
        conn.execute(
            r#"
            INSERT INTO catalog_packages (
                id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
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
    }

    #[test]
    fn package_queries_read_timestamps() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(CATALOG_SCHEMA)
            .expect("catalog schema should load");

        insert_catalog_package(&conn);

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

        insert_catalog_package(&conn);

        conn.execute(
            r#"
            UPDATE catalog_packages
            SET description = ?1
            WHERE id = ?2
            "#,
            params!["Updated package", "winget/Contoso.App"],
        )
        .expect("update catalog package");

        let package = get_package_by_id(&conn, "winget/Contoso.App")
            .expect("package lookup should succeed")
            .expect("package should exist");

        assert_eq!(package.description.as_deref(), Some("Updated package"));
        assert_ne!(package.updated_at.as_deref(), Some("2026-04-14 12:34:56"));
        assert_eq!(package.created_at.as_deref(), Some("2026-04-14 12:00:00"));
    }
}
