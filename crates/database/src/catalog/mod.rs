use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};

use super::CatalogSchemaVersionMismatchError;
use winbrew_models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION as CATALOG_SCHEMA_VERSION;

mod installers;
mod search;

pub use installers::get_installers;
pub use search::{get_package_by_id, search};

pub fn ensure_schema_version(conn: &Connection) -> Result<()> {
    let schema_meta_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_meta')",
        [],
        |row| row.get::<_, i64>(0),
    )? != 0;

    if !schema_meta_exists {
        return Err(anyhow::anyhow!(
            "package catalog schema metadata is missing"
        ));
    }

    let version_text: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE name = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context("failed to read package catalog schema version metadata")?;

    let version_text = version_text
        .ok_or_else(|| anyhow::anyhow!("package catalog schema version metadata is missing"))?;

    let actual_version: i64 = version_text.parse().with_context(|| {
        format!("failed to parse schema_meta schema_version value: {version_text}")
    })?;
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

#[cfg(test)]
mod tests {
    use super::{CATALOG_SCHEMA_VERSION, ensure_schema_version};
    use rusqlite::Connection;

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
        conn.execute_batch(
            "CREATE TABLE schema_meta (name TEXT PRIMARY KEY, value TEXT NOT NULL);\nINSERT INTO schema_meta (name, value) VALUES ('schema_version', '99');",
        )
        .expect("set mismatched schema version");

        let err = ensure_schema_version(&conn).expect_err("schema mismatch should fail");

        assert!(
            err.to_string()
                .contains("Package catalog schema version mismatch")
        );
    }

    #[test]
    fn ensure_schema_version_rejects_missing_schema_meta() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        let err = ensure_schema_version(&conn).expect_err("missing schema_meta should fail");

        assert!(
            err.to_string()
                .contains("package catalog schema metadata is missing")
        );
    }
}
