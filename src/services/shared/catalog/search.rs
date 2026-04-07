use anyhow::Result;
use rusqlite::Connection;

use crate::database;
use crate::models::CatalogPackage;

#[derive(Debug)]
pub enum SearchError {
    CatalogUnavailable,
    Unexpected(anyhow::Error),
}

pub type SearchResult<T> = std::result::Result<T, SearchError>;

impl From<anyhow::Error> for SearchError {
    fn from(value: anyhow::Error) -> Self {
        Self::Unexpected(value)
    }
}

pub fn search_catalog_packages(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    database::search(conn, query)
}

pub fn search_packages(query: &str) -> SearchResult<Vec<CatalogPackage>> {
    let conn = database::get_catalog_conn().map_err(SearchError::from)?;

    match database::search(&conn, query) {
        Ok(packages) => Ok(packages),
        Err(err)
            if err
                .downcast_ref::<database::CatalogNotFoundError>()
                .is_some() =>
        {
            Err(SearchError::CatalogUnavailable)
        }
        Err(err) => Err(SearchError::Unexpected(err)),
    }
}

pub fn resolve_catalog_package<FChoose>(
    conn: &Connection,
    query: &str,
    choose_package: &mut FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let matches = search_catalog_packages(conn, query)?;

    if matches.is_empty() {
        anyhow::bail!("no catalog packages matched '{query}'");
    }

    if matches.len() == 1 {
        return Ok(matches.into_iter().next().expect("single match exists"));
    }

    if let Some(exact_index) = matches
        .iter()
        .position(|pkg| pkg.name.eq_ignore_ascii_case(query))
    {
        return Ok(matches.into_iter().nth(exact_index).unwrap());
    }

    let selected = choose_package(query, &matches)?;

    matches
        .into_iter()
        .nth(selected)
        .ok_or_else(|| anyhow::anyhow!("selected package index was out of range"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use rusqlite::{Connection, params};
    use tempfile::tempdir;

    fn create_catalog_db() -> Result<(tempfile::TempDir, Connection)> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("catalog.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            r#"
            CREATE TABLE catalog_packages (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                version     TEXT NOT NULL,
                description TEXT,
                homepage    TEXT,
                license     TEXT,
                publisher   TEXT
            );

            CREATE VIRTUAL TABLE catalog_packages_fts USING fts5(
                name,
                description,
                content=catalog_packages,
                content_rowid=rowid
            );

            CREATE TRIGGER catalog_packages_ai AFTER INSERT ON catalog_packages BEGIN
                INSERT INTO catalog_packages_fts(rowid, name, description)
                VALUES (new.rowid, new.name, new.description);
            END;
            "#,
        )?;

        Ok((temp_dir, conn))
    }

    #[test]
    fn search_catalog_packages_returns_matches_from_catalog_db() -> Result<()> {
        let (_temp_dir, conn) = create_catalog_db()?;

        conn.execute(
            r#"
            INSERT INTO catalog_packages (
                id, name, version, description, homepage, license, publisher
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                "winget/Contoso.App",
                "Contoso Terminal",
                "1.2.3",
                Some("Terminal tools for Contoso users"),
                Option::<String>::None,
                Option::<String>::None,
                Some("Contoso Ltd."),
            ],
        )?;

        let matches = search_catalog_packages(&conn, "terminal")?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "winget/Contoso.App");
        assert_eq!(matches[0].name, "Contoso Terminal");
        assert_eq!(matches[0].source, "winget");

        Ok(())
    }
}
