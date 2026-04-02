use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::models::{CatalogInstaller, CatalogPackage};

pub fn search(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.version, p.source, p.description, p.homepage, p.license, p.publisher
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
        "SELECT package_id, url, hash, arch, type
         FROM catalog_installers
         WHERE package_id = ?1
         ORDER BY arch ASC, type ASC, url ASC",
    )?;

    stmt.query_map(params![package_id], row_to_installer)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read catalog installer")
}

fn row_to_package(row: &rusqlite::Row) -> rusqlite::Result<CatalogPackage> {
    Ok(CatalogPackage {
        id: row.get("id")?,
        name: row.get("name")?,
        version: row.get("version")?,
        source: row.get("source")?,
        description: row.get("description")?,
        homepage: row.get("homepage")?,
        license: row.get("license")?,
        publisher: row.get("publisher")?,
    })
}

fn row_to_installer(row: &rusqlite::Row) -> rusqlite::Result<CatalogInstaller> {
    Ok(CatalogInstaller {
        package_id: row.get("package_id")?,
        url: row.get("url")?,
        hash: row.get("hash")?,
        arch: row.get("arch")?,
        kind: row.get("type")?,
    })
}
