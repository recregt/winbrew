use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use zstd::stream::read::Decoder;

use crate::core::network::Client;

use super::metadata::{build_catalog_metadata_from_connection, verify_catalog_hash};

/// Applies one or more SQL patch files to an existing catalog database.
///
/// This is the incremental refresh path used when the update API returns a
/// patch plan. The function works on a temporary copy of the current catalog,
/// applies each patch URL in order, verifies the database integrity, rebuilds
/// catalog metadata, and validates the final catalog hash before writing the
/// updated metadata JSON.
///
/// # Workflow
/// 1. Confirm that the source catalog already exists.
/// 2. Copy the source catalog to the temporary patch working copy.
/// 3. Open the working copy with foreign keys enabled and `DELETE` journaling.
/// 4. Install a SQL authorizer that restricts patch SQL to plain data
///    reads/writes and refuses schema changes, `PRAGMA`, and `ATTACH`.
/// 5. Download and decompress each patch URL as zstd-compressed SQL.
/// 6. Execute each patch sequentially against the working copy, gated by the
///    authorizer installed in step 4.
/// 7. Remove the authorizer and run `PRAGMA integrity_check` to verify the
///    patched database.
/// 8. Rebuild metadata from the patched database state.
/// 9. Verify the patched database hash matches the rebuilt metadata hash.
/// 10. Write the refreshed metadata JSON to `metadata_temp_path`.
///
/// # Errors
/// Returns an error when the source catalog is missing, when the working copy
/// cannot be created or opened, when any patch download fails, when patch SQL
/// attempts an action the authorizer refuses, when any other SQL execution
/// fails, when the integrity check fails, when hash verification fails, or
/// when metadata serialization or writing fails.
///
/// # Safety
/// The source `catalog_path` is never modified directly. All patching happens
/// on the temporary working copy, which the caller finalizes separately.
///
/// The patch SQL itself comes from the update API response and is untrusted
/// network input: it is never given the ability to touch anything outside
/// ordinary row reads/writes on the working copy (see
/// [`install_patch_sql_authorizer`]), and the resulting database is still
/// hash-verified before it is allowed to replace the local catalog.
pub(super) fn apply_catalog_patch_release(
    client: &Client,
    catalog_path: &Path,
    catalog_temp_path: &Path,
    metadata_temp_path: &Path,
    patch_urls: &[String],
    expected_hash: &str,
    previous_hash: &str,
) -> Result<()> {
    if !catalog_path.exists() {
        bail!("cannot apply catalog patch without an existing catalog database");
    }

    fs::copy(catalog_path, catalog_temp_path)
        .context("failed to back up local catalog database for patch update")?;

    let connection =
        Connection::open(catalog_temp_path).context("failed to open catalog patch working copy")?;
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .context("failed to set catalog patch journal mode")?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .context("failed to enable foreign keys for catalog patch update")?;

    install_patch_sql_authorizer(&connection)
        .context("failed to install catalog patch SQL authorizer")?;

    let apply_result = (|| -> Result<()> {
        for patch_url in patch_urls {
            let patch_sql = download_catalog_patch_sql(client, patch_url)?;
            connection
                .execute_batch(&patch_sql)
                .with_context(|| format!("failed to apply catalog patch from {patch_url}"))?;
        }
        Ok(())
    })();

    // Everything from here on is trusted, internal SQLite housekeeping, not
    // patch-supplied SQL, so the restrictive authorizer is removed before it
    // runs anything else on this connection.
    connection
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
        .context("failed to remove catalog patch SQL authorizer")?;

    apply_result?;

    let integrity_check: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context("failed to run catalog integrity check after patch application")?;

    if integrity_check.trim() != "ok" {
        bail!("catalog integrity check failed after patch application: {integrity_check}");
    }

    let metadata =
        build_catalog_metadata_from_connection(&connection, expected_hash, previous_hash)?;

    drop(connection);

    verify_catalog_hash(catalog_temp_path, &metadata.current_hash)?;

    fs::write(
        metadata_temp_path,
        serde_json::to_vec_pretty(&metadata)
            .context("failed to serialize patched catalog metadata")?,
    )
    .context("failed to write patched catalog metadata")?;

    Ok(())
}

/// Installs a SQLite authorizer that restricts the connection to plain row
/// reads and writes for the duration it is active.
///
/// Catalog patch SQL comes from the update API response and is treated as
/// untrusted network input: an attacker who can spoof or compromise that
/// response can supply arbitrary SQL to [`Connection::execute_batch`], which
/// otherwise permits schema changes, `PRAGMA` statements, and
/// `ATTACH DATABASE '<path>' AS ...` -- the last of which lets arbitrary SQL
/// create or overwrite a file anywhere on disk that the process can write to.
/// A legitimate patch only ever needs to insert, update, delete, or read rows
/// in the existing catalog schema, so everything else is refused before it
/// can run rather than being caught after the fact by the final hash check.
///
/// The authorizer must be removed (`connection.authorizer(None)`) before the
/// connection is used for anything else, since SQLite invokes it for every
/// statement prepared while it is installed, including trusted internal
/// housekeeping like `PRAGMA integrity_check`.
fn install_patch_sql_authorizer(connection: &Connection) -> Result<()> {
    connection.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Select
        | AuthAction::Read { .. }
        | AuthAction::Insert { .. }
        | AuthAction::Update { .. }
        | AuthAction::Delete { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. }
        | AuthAction::Function { .. }
        | AuthAction::Recursive => Authorization::Allow,
        // SQLite (via rusqlite's `unlock_notify` support) queries this
        // read-only, valueless PRAGMA internally while preparing ordinary
        // statements, independent of what the patch SQL itself says. It only
        // reports a schema-change counter and has no filesystem or schema
        // side effect, so it is allowed alongside the plain data actions
        // above rather than lumped in with the catch-all below.
        AuthAction::Pragma {
            pragma_name: "data_version",
            pragma_value: None,
        } => Authorization::Allow,
        // Schema changes, every other PRAGMA, ATTACH/DETACH, virtual
        // tables, and anything else has no legitimate role in a catalog
        // data patch.
        _ => Authorization::Deny,
    }))?;

    Ok(())
}

/// Downloads and decompresses a single zstd-compressed catalog patch SQL file.
///
/// The patch payload is read fully into memory, decompressed, and returned as
/// UTF-8 SQL text ready for execution against the working copy database.
///
/// # Errors
/// Returns an error when the HTTP request fails, when the server returns a
/// non-success status, when the response body cannot be read, when the payload
/// cannot be decompressed, or when the SQL text cannot be decoded.
fn download_catalog_patch_sql(client: &Client, patch_url: &str) -> Result<String> {
    let response = client
        .get(patch_url.to_string())
        .send()
        .with_context(|| format!("failed to send catalog patch request to {patch_url}"))?;
    let response = response
        .error_for_status()
        .with_context(|| format!("catalog patch request failed for {patch_url}"))?;

    let patch_bytes = response
        .bytes()
        .with_context(|| format!("failed to read catalog patch response from {patch_url}"))?;

    let mut decoder = Decoder::new(Cursor::new(patch_bytes))
        .context("failed to decompress catalog patch payload")?;
    let mut patch_sql = String::new();
    decoder
        .read_to_string(&mut patch_sql)
        .context("failed to decode catalog patch SQL")?;

    Ok(patch_sql)
}

#[cfg(test)]
mod tests {
    use super::install_patch_sql_authorizer;
    use rusqlite::Connection;

    const CATALOG_SCHEMA: &str = include_str!("../../../../../infra/parser/schema/catalog.sql");

    fn open_catalog_with_authorizer() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory catalog");
        connection
            .execute_batch(CATALOG_SCHEMA)
            .expect("load catalog schema");
        install_patch_sql_authorizer(&connection).expect("install patch sql authorizer");
        connection
    }

    #[test]
    fn allows_ordinary_data_statements() {
        let connection = open_catalog_with_authorizer();

        // Insert and delete a row through the authorizer to prove plain DML
        // is allowed. (An UPDATE against `catalog_packages` is deliberately
        // not exercised here: it trips an unrelated, pre-existing FTS5
        // external-content trigger interaction that reproduces with plain
        // `sqlite3`/rusqlite and no authorizer involved at all -- the
        // `refresh_catalog_applies_api_selected_patches` integration test in
        // `crates/app/tests/update_refresh_tests.rs` already exercises a real
        // UPDATE-free INSERT patch end-to-end through this same authorizer.)
        connection
            .execute_batch(
                "INSERT INTO catalog_packages (id, name, version, source, source_id, locale, created_at, updated_at)
                 VALUES ('winget/Contoso.App', 'Contoso App', '1.0.0', 'winget', 'Contoso.App', 'en-US', '2026-04-14 12:00:00', '2026-04-14 12:00:00');",
            )
            .expect("insert patch should be allowed");
        connection
            .execute_batch("DELETE FROM catalog_packages WHERE id = 'winget/Contoso.App';")
            .expect("delete patch should be allowed");
    }

    #[test]
    fn rejects_attach_database() {
        let connection = open_catalog_with_authorizer();

        let err = connection
            .execute_batch("ATTACH DATABASE '/tmp/winbrew-authorizer-test.db' AS evil;")
            .expect_err("ATTACH should be refused by the patch authorizer");

        assert!(err.to_string().to_lowercase().contains("not authorized"));
    }

    #[test]
    fn rejects_pragma_statements() {
        let connection = open_catalog_with_authorizer();

        connection
            .execute_batch("PRAGMA journal_mode = WAL;")
            .expect_err("PRAGMA writes should be refused by the patch authorizer");
    }

    #[test]
    fn rejects_schema_changes() {
        let connection = open_catalog_with_authorizer();

        connection
            .execute_batch("CREATE TABLE evil (id INTEGER);")
            .expect_err("DDL should be refused by the patch authorizer");
    }
}
