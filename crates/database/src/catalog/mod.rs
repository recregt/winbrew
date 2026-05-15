use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OptionalExtension};

use super::CatalogSchemaVersionMismatchError;
use crate::models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION as CATALOG_SCHEMA_VERSION;

mod installers;
mod search;

pub use installers::get_installers;
pub use search::{get_package_by_id, search};

pub(crate) fn conversion_err<E>(err: E) -> rusqlite::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    // Column index is not surfaced in our error path; 0 is a conventional placeholder.
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
}

pub fn ensure_schema_version(conn: &Connection) -> Result<()> {
    let version_text: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE name = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context("failed to read package catalog schema version metadata")?;

    let version_text = version_text
        .ok_or_else(|| anyhow!("package catalog schema version metadata is missing"))?;

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
            "
            CREATE TABLE schema_meta (
                name TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT INTO schema_meta (name, value)
            VALUES ('schema_version', '{}');
            ",
            CATALOG_SCHEMA_VERSION
        ))
        .expect("set schema version");

        ensure_schema_version(&conn).expect("schema version should match");
    }

    #[test]
    fn ensure_schema_version_rejects_mismatch() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(
            "
            CREATE TABLE schema_meta (
                name TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT INTO schema_meta (name, value)
            VALUES ('schema_version', '99');
            ",
        )
        .expect("set mismatched schema version");

        let err = ensure_schema_version(&conn).expect_err("schema mismatch should fail");

        assert!(
            err.to_string()
                .contains("Package catalog schema version mismatch")
        );
    }

    #[test]
    fn ensure_schema_version_rejects_missing_schema_version_row() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        conn.execute_batch(
            "
            CREATE TABLE schema_meta (
                name TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .expect("create schema_meta table");

        let err = ensure_schema_version(&conn).expect_err("missing schema version row should fail");

        assert!(
            err.to_string()
                .contains("package catalog schema version metadata is missing")
        );
    }

    #[test]
    fn ensure_schema_version_rejects_missing_schema_meta_table() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        let err = ensure_schema_version(&conn).expect_err("missing schema_meta table should fail");

        assert!(
            err.to_string()
                .contains("failed to read package catalog schema version metadata")
        );
    }
}
