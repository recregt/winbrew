use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::path::Path;

use crate::core::hash::{hash_file, verify_hash};
use crate::models::catalog::CatalogMetadata;
use crate::models::domains::shared::HashAlgorithm;

pub(super) fn load_local_catalog_metadata(path: &Path) -> Result<Option<CatalogMetadata>> {
    match fs::metadata(path) {
        Ok(_) => load_catalog_metadata(path).map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("failed to inspect local catalog metadata"),
    }
}

pub(super) fn load_catalog_metadata(path: &Path) -> Result<CatalogMetadata> {
    let file = File::open(path).context("failed to open catalog metadata download")?;
    let metadata: CatalogMetadata =
        serde_json::from_reader(file).context("failed to decode catalog metadata download")?;
    metadata.validate()?;

    Ok(metadata)
}

pub(super) fn verify_catalog_hash(path: &Path, expected_hash: &str) -> Result<()> {
    let actual_hash = hash_file(path, HashAlgorithm::Sha256)
        .context("failed to hash downloaded catalog database")?;

    verify_hash(expected_hash, actual_hash).map_err(Into::into)
}

pub(super) fn metadata_url_for_snapshot_url(snapshot_url: &str) -> Result<String> {
    let (prefix, _) = snapshot_url
        .rsplit_once('/')
        .context("snapshot URL must contain a path segment")?;

    Ok(format!("{prefix}/metadata.json"))
}

pub(super) fn build_catalog_metadata_from_connection(
    connection: &Connection,
    current_hash: &str,
    previous_hash: &str,
) -> Result<CatalogMetadata> {
    let package_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM catalog_packages", [], |row| {
            row.get(0)
        })
        .context("failed to count catalog packages")?;
    let package_count =
        usize::try_from(package_count).context("catalog package count does not fit in usize")?;

    let mut source_counts = BTreeMap::new();
    let mut stmt = connection
        .prepare(
            "SELECT source, COUNT(*) FROM catalog_packages GROUP BY source ORDER BY source ASC",
        )
        .context("failed to prepare catalog source count query")?;
    let mut rows = stmt
        .query([])
        .context("failed to query catalog source counts")?;

    while let Some(row) = rows
        .next()
        .context("failed to read catalog source count row")?
    {
        let source: String = row.get(0).context("failed to read catalog source name")?;
        let count: i64 = row.get(1).context("failed to read catalog source count")?;
        let count = usize::try_from(count).context("catalog source count does not fit in usize")?;
        source_counts.insert(source, count);
    }

    let mut metadata =
        CatalogMetadata::build_from_counts(package_count, source_counts, current_hash.to_string());
    metadata.previous_hash = previous_hash.to_string();
    metadata.validate()?;

    Ok(metadata)
}
