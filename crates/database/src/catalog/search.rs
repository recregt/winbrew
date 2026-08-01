use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use super::row::conversion_err;
use crate::models::catalog::package::CatalogPackage;

/// Search catalog packages by a full-text query.
///
/// Blank or whitespace-only queries return an empty result set.
///
/// # Errors
///
/// Returns an error if SQLite query execution or row conversion fails.
pub(crate) fn search(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let fts_query = fts5_match_query(query);

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.version, p.source, p.namespace, p.source_id, p.created_at, p.updated_at, p.description, p.homepage, p.license, p.publisher, p.locale, p.moniker, p.platform, p.commands, p.protocols, p.file_extensions, p.capabilities, p.tags, p.bin, p.env_add_path
         FROM catalog_packages p
         JOIN catalog_packages_fts fts ON p.rowid = fts.rowid
         WHERE catalog_packages_fts MATCH ?1
            ORDER BY bm25(catalog_packages_fts, 10.0, 5.0, 6.0, 1.0), p.name ASC",
    )?;

    stmt.query_map(params![fts_query], row_to_package)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read catalog package")
}

/// Build a safe FTS5 `MATCH` query from free-text user input.
///
/// Binding a raw user string as an FTS5 `MATCH` argument is not the same as
/// binding it into an ordinary SQL predicate: the bound value is itself
/// parsed by SQLite's FTS5 query-syntax parser, independent of normal SQL
/// parameter escaping. Reserved tokens (`AND`, `OR`, `NOT`), grouping and
/// prefix operators (`(`, `)`, `-`, `*`), and the `column:` filter syntax
/// are all live syntax in that mini-language, so an unescaped search term
/// like `foo AND` or an unbalanced `"` either throws an opaque FTS5 syntax
/// error at the user or silently changes what the query means.
///
/// Every whitespace-separated term is wrapped as its own FTS5 string
/// literal (embedded `"` doubled per FTS5's escaping rule), which forces
/// each term to be matched as literal text rather than parsed as syntax.
/// Adjacent string literals with no explicit operator between them still
/// combine with FTS5's default implicit AND, so this preserves the
/// existing multi-word search behavior for ordinary queries.
fn fts5_match_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return a single catalog package by its catalog package id.
///
/// # Errors
///
/// Returns an error if SQLite query execution or row conversion fails.
pub(crate) fn get_package_by_id(
    conn: &Connection,
    package_id: &str,
) -> Result<Option<CatalogPackage>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, version, source, namespace, source_id, created_at, updated_at, description, homepage, license, publisher, locale, moniker, platform, commands, protocols, file_extensions, capabilities, tags, bin, env_add_path
         FROM catalog_packages
         WHERE id = ?1",
    )?;

    stmt.query_row(params![package_id], row_to_package)
        .optional()
        .context("failed to read catalog package")
}

fn row_to_package(row: &rusqlite::Row) -> rusqlite::Result<CatalogPackage> {
    let version = row
        .get::<_, String>("version")?
        .parse()
        .map_err(conversion_err)?;
    let source = row
        .get::<_, String>("source")?
        .parse()
        .map_err(conversion_err)?;

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
        env_add_path: row.get("env_add_path")?,
    };

    package.validate().map_err(conversion_err)?;

    Ok(package)
}

#[cfg(test)]
mod tests {
    use super::{fts5_match_query, get_package_by_id, search};
    use rusqlite::{Connection, params};

    const CATALOG_SCHEMA: &str = include_str!("../../../../infra/parser/schema/catalog.sql");

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(CATALOG_SCHEMA)
            .expect("catalog schema should load");
        conn
    }

    fn insert_catalog_package(
        conn: &Connection,
        id: &str,
        name: &str,
        description: Option<&str>,
        moniker: Option<&str>,
        tags: Option<&str>,
    ) {
        conn.execute(
            r#"
            INSERT INTO catalog_packages (
                id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                id,
                name,
                "1.2.3",
                "winget",
                None::<String>,
                id.split('/').nth(1).unwrap_or(id),
                description,
                None::<String>,
                None::<String>,
                Some("Example publisher"),
                Some("en-US"),
                moniker,
                tags,
                "2026-04-14 12:00:00",
                "2026-04-14 12:34:56",
            ],
        )
        .expect("insert catalog package");
    }

    #[test]
    fn package_queries_read_timestamps() {
        let conn = open_test_db();

        insert_catalog_package(
            &conn,
            "winget/Contoso.App",
            "Contoso App",
            Some("Example package"),
            None,
            None,
        );

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
        let conn = open_test_db();

        insert_catalog_package(
            &conn,
            "winget/Contoso.App",
            "Contoso App",
            Some("Example package"),
            None,
            None,
        );

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
        let updated_at = package
            .updated_at
            .as_deref()
            .expect("package should have updated_at");
        assert!(updated_at > "2026-04-14 12:34:56");
        assert_eq!(package.created_at.as_deref(), Some("2026-04-14 12:00:00"));
    }

    #[test]
    fn package_queries_read_env_add_path() {
        let conn = open_test_db();

        insert_catalog_package(
            &conn,
            "winget/Contoso.App",
            "Contoso App",
            Some("Example package"),
            None,
            None,
        );

        conn.execute(
            r#"
            UPDATE catalog_packages
            SET env_add_path = ?1
            WHERE id = ?2
            "#,
            params![r#"["bin","tools"]"#, "winget/Contoso.App"],
        )
        .expect("update env_add_path");

        let package = get_package_by_id(&conn, "winget/Contoso.App")
            .expect("package lookup should succeed")
            .expect("package should exist");

        assert_eq!(package.env_add_path.as_deref(), Some("[\"bin\",\"tools\"]"));
    }

    #[test]
    fn search_matches_accentless_queries_against_diacritics() {
        let conn = open_test_db();

        insert_catalog_package(
            &conn,
            "winget/CocCoc.Browser",
            "Cốc Cốc",
            Some("Vietnamese browser"),
            None,
            Some(r#"["browser"]"#),
        );

        let searched = search(&conn, "coc").expect("catalog search should succeed");

        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].name, "Cốc Cốc");
    }

    #[test]
    fn search_prioritizes_name_matches_over_tag_noise() {
        let conn = open_test_db();

        insert_catalog_package(
            &conn,
            "winget/Google.Chrome",
            "Google Chrome",
            Some("Web browser"),
            None,
            Some(r#"["browser"]"#),
        );
        insert_catalog_package(
            &conn,
            "winget/NodeJs.ChromeNoise",
            "NodeJS",
            Some("JavaScript runtime"),
            None,
            Some(r#"["chrome", "chrome", "chrome"]"#),
        );

        let searched = search(&conn, "chrome").expect("catalog search should succeed");

        assert_eq!(searched.len(), 2);
        assert_eq!(searched[0].name, "Google Chrome");
        assert_eq!(searched[1].name, "NodeJS");
    }

    #[test]
    fn fts5_match_query_wraps_each_term_as_a_literal() {
        assert_eq!(fts5_match_query("chrome"), "\"chrome\"");
        assert_eq!(fts5_match_query("Visual Studio"), "\"Visual\" \"Studio\"");
        assert_eq!(fts5_match_query("  a   b  "), "\"a\" \"b\"");
    }

    #[test]
    fn fts5_match_query_escapes_embedded_quotes_and_defuses_operators() {
        assert_eq!(fts5_match_query(r#"foo"bar"#), "\"foo\"\"bar\"");
        assert_eq!(fts5_match_query("NOT"), "\"NOT\"");
        assert_eq!(fts5_match_query("foo AND"), "\"foo\" \"AND\"");
        assert_eq!(fts5_match_query("(foo)"), "\"(foo)\"");
        assert_eq!(fts5_match_query("name:foo"), "\"name:foo\"");
    }

    /// Special FTS5 syntax characters (quotes, boolean keywords, grouping,
    /// column filters) used to reach `MATCH` unescaped, so they were parsed
    /// as query syntax instead of literal search text -- an unbalanced `"`
    /// or a bare `NOT`/`AND`/`OR` term threw an FTS5 syntax error straight
    /// at the user instead of behaving like an ordinary (non-matching)
    /// search. This locks in that such input now searches safely.
    #[test]
    fn search_handles_fts5_special_characters_without_erroring() {
        let conn = open_test_db();

        insert_catalog_package(
            &conn,
            "winget/Contoso.App",
            "Contoso App",
            Some("Example package"),
            None,
            None,
        );

        for query in [
            "\"unbalanced quote",
            "NOT",
            "AND OR",
            "foo AND",
            "(unterminated",
            "name:contoso",
            "foo*",
            "foo\"bar",
            "-contoso",
        ] {
            search(&conn, query)
                .unwrap_or_else(|err| panic!("search for {query:?} should not error: {err}"));
        }

        let matched = search(&conn, "name:contoso").expect("catalog search should succeed");
        assert!(
            matched.is_empty(),
            "column-filter syntax should be treated as literal text, not a column filter"
        );

        let matched = search(&conn, "Contoso").expect("catalog search should succeed");
        assert_eq!(matched.len(), 1);
    }
}
