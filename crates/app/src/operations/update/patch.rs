use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use zstd::stream::read::Decoder;

use crate::core::network::Client;

use super::metadata::{build_catalog_metadata_from_connection, verify_catalog_hash};

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
