use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::path::Path;

use crate::core::hash::{hash_file, verify_hash};
use crate::models::catalog::CatalogMetadata;
use crate::models::domains::shared::HashAlgorithm;
use url::Url;

/// Tries to load the local catalog metadata file if it already exists.
///
/// This is the refresh-path entry point for on-disk metadata. It first checks
/// whether `path` exists and only attempts to open and validate the file when
/// the file is present.
///
/// Returns `Ok(None)` when the file is missing, which is the normal cold-start
/// case for a first refresh. Any other filesystem, parse, or validation failure
/// is returned as an error so callers can surface the problem.
pub(super) fn load_local_catalog_metadata(path: &Path) -> Result<Option<CatalogMetadata>> {
    match fs::metadata(path) {
        Ok(_) => load_catalog_metadata(path).map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("failed to inspect local catalog metadata"),
    }
}

/// Loads catalog metadata from a JSON file and validates the decoded payload.
///
/// The function expects a serialized `CatalogMetadata` document, deserializes
/// it with `serde_json`, and then runs the model-level validation rules before
/// returning the value to the caller.
///
/// Failures are reported when the file cannot be opened, when JSON decoding
/// fails, or when the decoded metadata does not satisfy the model validation
/// rules.
pub(super) fn load_catalog_metadata(path: &Path) -> Result<CatalogMetadata> {
    let file = File::open(path).context("failed to open catalog metadata download")?;
    let metadata: CatalogMetadata =
        serde_json::from_reader(file).context("failed to decode catalog metadata download")?;
    metadata.validate()?;

    Ok(metadata)
}

/// Verifies that the catalog database at `path` matches `expected_hash`.
///
/// The file is hashed with SHA-256 and compared against the expected digest in
/// the existing `verify_hash` format used across the workspace.
///
/// This is the last integrity check before the refreshed catalog is finalized,
/// so any mismatch is treated as a hard error.
pub(super) fn verify_catalog_hash(path: &Path, expected_hash: &str) -> Result<()> {
    let actual_hash = hash_file(path, HashAlgorithm::Sha256)
        .context("failed to hash downloaded catalog database")?;

    verify_hash(expected_hash, actual_hash).map_err(Into::into)
}

/// Derives the metadata URL for a snapshot download URL.
///
/// The function parses `snapshot_url`, keeps the original scheme and host, and
/// replaces the final path segment with `metadata.json`. For example,
/// `https://cdn.example.invalid/releases/catalog.db.zst` becomes
/// `https://cdn.example.invalid/releases/metadata.json`.
///
/// An error is returned when the input is not a valid URL or when the URL does
/// not contain a non-empty final path segment to replace.
pub(super) fn metadata_url_for_snapshot_url(snapshot_url: &str) -> Result<String> {
    let mut url = Url::parse(snapshot_url).context("invalid snapshot URL")?;
    let path = url.path();
    let (base_path, file_name) = path
        .rsplit_once('/')
        .context("snapshot URL must contain a path segment")?;

    if file_name.is_empty() {
        anyhow::bail!("snapshot URL must contain a path segment");
    }

    url.set_path(&format!("{base_path}/metadata.json"));

    Ok(url.to_string())
}

/// Builds validated catalog metadata from the live SQLite catalog database.
///
/// The function reads the total package count and the per-source breakdown from
/// `catalog_packages`, then packages those counts together with the supplied
/// current and previous hashes. The resulting metadata is validated before it
/// is returned.
///
/// The query shape is intentionally simple and explicit: one total-count query
/// and one grouped source-count query. That keeps the logic easy to audit and
/// matches the schema contract used by the refresh pipeline.
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

#[cfg(test)]
mod tests {
    use super::{load_local_catalog_metadata, metadata_url_for_snapshot_url};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn load_local_catalog_metadata_returns_none_when_file_is_missing() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("metadata.json");

        let loaded = load_local_catalog_metadata(&path).expect("load local metadata");

        assert!(loaded.is_none());
    }

    #[test]
    fn load_local_catalog_metadata_rejects_invalid_json() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("metadata.json");

        fs::write(&path, b"not valid json").expect("write invalid metadata");

        let err = load_local_catalog_metadata(&path).expect_err("invalid metadata should fail");

        assert!(
            err.to_string()
                .contains("failed to decode catalog metadata download")
        );
    }

    #[test]
    fn metadata_url_for_snapshot_url_rejects_missing_path_segment() {
        let err = metadata_url_for_snapshot_url("https://cdn.example.invalid")
            .expect_err("snapshot url without path should fail");

        assert!(
            err.to_string()
                .contains("snapshot URL must contain a path segment")
        );
    }
}
