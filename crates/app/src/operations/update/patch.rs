use anyhow::{Context, Result, bail};
use rusqlite::Connection;
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
/// 4. Download and decompress each patch URL as zstd-compressed SQL.
/// 5. Execute each patch sequentially against the working copy.
/// 6. Run `PRAGMA integrity_check` to verify the patched database.
/// 7. Rebuild metadata from the patched database state.
/// 8. Verify the patched database hash matches the rebuilt metadata hash.
/// 9. Write the refreshed metadata JSON to `metadata_temp_path`.
///
/// # Errors
/// Returns an error when the source catalog is missing, when the working copy
/// cannot be created or opened, when any patch download or SQL execution
/// fails, when the integrity check fails, when hash verification fails, or
/// when metadata serialization or writing fails.
///
/// # Safety
/// The source `catalog_path` is never modified directly. All patching happens
/// on the temporary working copy, which the caller finalizes separately.
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

    for patch_url in patch_urls {
        let patch_sql = download_catalog_patch_sql(client, patch_url)?;
        connection
            .execute_batch(&patch_sql)
            .with_context(|| format!("failed to apply catalog patch from {patch_url}"))?;
    }

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
