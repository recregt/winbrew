//! Catalog refresh workflow for the CLI.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use crate::core::fs::finalize_temp_file;
use crate::core::hash::{Hasher, verify_hash};
use crate::core::network::{Client, build_client, download_url_to_temp_file};
use crate::core::paths::ResolvedPaths;
use crate::models::{CatalogMetadata, HashAlgorithm};

const CATALOG_DIRECT_DOWNLOAD_URL: &str =
    "https://github.com/recregt/winbrew/releases/latest/download/catalog.db";
const CATALOG_METADATA_DIRECT_DOWNLOAD_URL: &str =
    "https://github.com/recregt/winbrew/releases/latest/download/metadata.json";

pub fn refresh_catalog<FStart, FProgress>(
    paths: &ResolvedPaths,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let catalog_path = paths.catalog_db.clone();
    let catalog_dir = catalog_path
        .parent()
        .context("failed to resolve catalog database directory")?;

    fs::create_dir_all(catalog_dir).context("failed to create catalog database directory")?;

    let catalog_temp_path = catalog_dir.join("catalog.db.download");
    let metadata_temp_path = catalog_dir.join("metadata.json.download");

    let result = (|| -> Result<()> {
        clear_temp_file(&catalog_temp_path)?;
        clear_temp_file(&metadata_temp_path)?;

        let client = build_client("winbrew-catalog-downloader")?;
        let metadata = download_catalog_metadata_release(&client, &metadata_temp_path)?;

        download_catalog_release(&client, &catalog_temp_path, on_start, on_progress)?;
        verify_catalog_hash(&catalog_temp_path, &metadata.current_hash)?;

        finalize_temp_file(&catalog_temp_path, &catalog_path)?;

        Ok(())
    })();

    let _ = fs::remove_file(&catalog_temp_path);
    let _ = fs::remove_file(&metadata_temp_path);

    result
}

fn clear_temp_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).context("failed to clear previous catalog download")?;
    }

    Ok(())
}

fn download_catalog_metadata_release(client: &Client, temp_path: &Path) -> Result<CatalogMetadata> {
    download_url_to_temp_file(
        client,
        CATALOG_METADATA_DIRECT_DOWNLOAD_URL,
        temp_path,
        "catalog metadata asset",
        |_| {},
        |_| {},
        |_| Ok(()),
    )?;

    load_catalog_metadata(temp_path)
}

fn download_catalog_release<FStart, FProgress>(
    client: &Client,
    temp_path: &Path,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    Ok(download_url_to_temp_file(
        client,
        CATALOG_DIRECT_DOWNLOAD_URL,
        temp_path,
        "catalog asset",
        on_start,
        on_progress,
        |_| Ok(()),
    )?)
}

fn load_catalog_metadata(path: &Path) -> Result<CatalogMetadata> {
    let file = File::open(path).context("failed to open catalog metadata download")?;
    let metadata: CatalogMetadata =
        serde_json::from_reader(file).context("failed to decode catalog metadata download")?;
    metadata.validate()?;

    Ok(metadata)
}

fn verify_catalog_hash(path: &Path, expected_hash: &str) -> Result<()> {
    let mut file = File::open(path).context("failed to open downloaded catalog database")?;
    let mut hasher = Hasher::new(HashAlgorithm::Sha256);
    let mut buffer = [0u8; 256 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .context("failed to read downloaded catalog database")?;
        if read == 0 {
            break;
        }

        hasher.update(&buffer[..read]);
    }

    verify_hash(expected_hash, hasher.finalize()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{load_catalog_metadata, verify_catalog_hash};
    use crate::core::hash::Hasher;
    use crate::models::CatalogMetadata;
    use crate::models::HashAlgorithm;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Hasher::new(HashAlgorithm::Sha256);
        hasher.update(bytes);

        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[test]
    fn load_catalog_metadata_reads_valid_metadata() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("metadata.json");
        let metadata = CatalogMetadata::build_from_counts(
            2,
            BTreeMap::from([(String::from("scoop"), 1)]),
            String::from("sha256:abc"),
        );

        fs::write(
            &path,
            serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
        )
        .expect("write metadata");

        let loaded = load_catalog_metadata(&path).expect("load metadata");

        assert_eq!(loaded.current_hash, metadata.current_hash);
        assert_eq!(loaded.package_count, metadata.package_count);
        assert_eq!(loaded.source_counts.get("scoop"), Some(&1));
    }

    #[test]
    fn verify_catalog_hash_accepts_matching_hash() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("catalog.db");
        let contents = b"catalog-bytes";

        fs::write(&path, contents).expect("write catalog");

        let expected_hash = format!("sha256:{}", sha256_hex(contents));

        verify_catalog_hash(&path, &expected_hash).expect("hash should match");
    }

    #[test]
    fn verify_catalog_hash_rejects_mismatch() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("catalog.db");

        fs::write(&path, b"catalog-bytes").expect("write catalog");

        let err = verify_catalog_hash(
            &path,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect_err("hash mismatch should fail");

        assert!(err.to_string().contains("checksum mismatch"));
    }
}
