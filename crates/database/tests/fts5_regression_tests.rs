use anyhow::Result;
use rusqlite::{Connection, params};
use winbrew_database as database;

const CATALOG_SCHEMA: &str = include_str!("../../../infra/parser/schema/catalog.sql");

fn open_catalog() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(CATALOG_SCHEMA)?;
    Ok(conn)
}

fn insert_package(
    conn: &Connection,
    package_id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO catalog_packages (
            id, name, version, source, namespace, source_id, description, homepage, license, publisher
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        params![
            package_id,
            name,
            "1.0.0",
            "winget",
            Option::<String>::None,
            package_id.trim_start_matches("winget/"),
            description.map(str::to_string),
            Option::<String>::None,
            Option::<String>::None,
            Option::<String>::None,
        ],
    )?;

    Ok(())
}

#[test]
fn insert_syncs_to_fts5_and_preserves_hidden_rowid() -> Result<()> {
    let conn = open_catalog()?;
    insert_package(
        &conn,
        "winget/Microsoft.VisualStudioCode",
        "Visual Studio Code",
        None,
    )?;

    let package_rowid: i64 = conn.query_row(
        "SELECT rowid FROM catalog_packages WHERE id = ?1",
        params!["winget/Microsoft.VisualStudioCode"],
        |row| row.get(0),
    )?;

    let fts_rowid: i64 = conn.query_row(
        "SELECT rowid FROM catalog_packages_fts WHERE catalog_packages_fts MATCH ?1",
        params!["Visual"],
        |row| row.get(0),
    )?;

    assert_eq!(fts_rowid, package_rowid);

    let results = database::search(&conn, "Visual Studio")?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id.as_ref(), "winget/Microsoft.VisualStudioCode");
    assert_eq!(results[0].name, "Visual Studio Code");

    Ok(())
}

#[test]
fn update_rewrites_fts5_index() -> Result<()> {
    let conn = open_catalog()?;
    insert_package(
        &conn,
        "winget/Microsoft.VisualStudioCode",
        "Visual Studio Code",
        Some("Code editor"),
    )?;

    let package_rowid_before: i64 = conn.query_row(
        "SELECT rowid FROM catalog_packages WHERE id = ?1",
        params!["winget/Microsoft.VisualStudioCode"],
        |row| row.get(0),
    )?;

    conn.execute(
        "UPDATE catalog_packages SET name = ?1 WHERE id = ?2",
        params!["VS Code", "winget/Microsoft.VisualStudioCode"],
    )?;

    let package_rowid_after: i64 = conn.query_row(
        "SELECT rowid FROM catalog_packages WHERE id = ?1",
        params!["winget/Microsoft.VisualStudioCode"],
        |row| row.get(0),
    )?;

    assert_eq!(package_rowid_after, package_rowid_before);
    assert!(database::search(&conn, "Visual Studio")?.is_empty());

    let results = database::search(&conn, "VS Code")?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id.as_ref(), "winget/Microsoft.VisualStudioCode");
    assert_eq!(results[0].name, "VS Code");

    let fts_rowid: i64 = conn.query_row(
        "SELECT rowid FROM catalog_packages_fts WHERE catalog_packages_fts MATCH ?1",
        params!["VS"],
        |row| row.get(0),
    )?;

    assert_eq!(fts_rowid, package_rowid_after);

    Ok(())
}

#[test]
fn delete_removes_fts5_entries() -> Result<()> {
    let conn = open_catalog()?;
    insert_package(
        &conn,
        "winget/Contoso.Editor",
        "Contoso Editor",
        Some("A lightweight editor"),
    )?;

    assert_eq!(database::search(&conn, "Contoso")?.len(), 1);

    conn.execute(
        "DELETE FROM catalog_packages WHERE id = ?1",
        params!["winget/Contoso.Editor"],
    )?;

    assert!(database::search(&conn, "Contoso")?.is_empty());

    let remaining: i64 = conn.query_row(
        "SELECT COUNT(*) FROM catalog_packages_fts WHERE catalog_packages_fts MATCH ?1",
        params!["Contoso"],
        |row| row.get(0),
    )?;

    assert_eq!(remaining, 0);

    Ok(())
}
