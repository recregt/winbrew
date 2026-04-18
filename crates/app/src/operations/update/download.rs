use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use zstd::stream::read::Decoder;

use crate::core::network::{Client, download_url_to_temp_file};

use super::metadata::{load_catalog_metadata, verify_catalog_hash};
use super::types::CatalogDownloadPlan;

/// Downloads a full catalog snapshot through the two-stage update flow.
///
/// The function keeps the refresh sequence strict and explicit:
/// 1. download the metadata JSON into `metadata_temp_path`
/// 2. parse the metadata and confirm its `current_hash` when the plan carries an expected hash
/// 3. download the compressed catalog snapshot into a sibling `.zst` temp file
/// 4. decompress the snapshot into `catalog_temp_path`
/// 5. verify the final database hash against the metadata hash
///
/// The caller owns the final metadata and catalog temp files. This function only
/// removes the internal compressed snapshot temp file it creates while
/// processing the release.
///
/// # Errors
/// Returns an error when the plan is not `Full`, when either download fails,
/// when the metadata hash does not match the expected hash, when decompression
/// fails, or when the final catalog hash check fails.
pub(super) fn download_catalog_release<FStart, FProgress>(
    client: &Client,
    plan: &CatalogDownloadPlan,
    catalog_temp_path: &Path,
    metadata_temp_path: &Path,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let CatalogDownloadPlan::Full {
        catalog_url,
        metadata_url,
        expected_hash,
    } = plan
    else {
        bail!("download_catalog_release only supports full snapshot plans");
    };

    let compressed_catalog_temp_path = compressed_snapshot_temp_path(catalog_temp_path);

    let result = (|| -> Result<()> {
        download_url_to_temp_file(
            client,
            metadata_url,
            metadata_temp_path,
            "catalog metadata asset",
            |_| {},
            |_| {},
            |_| Ok(()),
        )?;

        let metadata = load_catalog_metadata(metadata_temp_path)?;

        if let Some(expected_hash) = expected_hash
            && metadata.current_hash.as_str() != expected_hash.as_str()
        {
            bail!(
                "catalog metadata hash mismatch: expected {expected_hash}, got {}",
                metadata.current_hash
            );
        }

        download_url_to_temp_file(
            client,
            catalog_url,
            &compressed_catalog_temp_path,
            "catalog asset",
            on_start,
            on_progress,
            |_| Ok(()),
        )?;

        decompress_catalog_snapshot(&compressed_catalog_temp_path, catalog_temp_path)?;
        verify_catalog_hash(catalog_temp_path, &metadata.current_hash)?;

        Ok(())
    })();

    let _ = std::fs::remove_file(&compressed_catalog_temp_path);

    result
}

fn compressed_snapshot_temp_path(catalog_temp_path: &Path) -> PathBuf {
    let file_name = catalog_temp_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("catalog.db.download");

    catalog_temp_path.with_file_name(format!("{file_name}.zst"))
}

/// Decompresses a Zstandard-compressed catalog snapshot into `output_path`.
///
/// The file is streamed through a buffered writer and then flushed and synced so
/// a successful return means the temporary output is fully materialized on disk.
///
/// # Errors
/// Returns an error if the compressed input cannot be opened, if the decoder
/// cannot be created, if the output file cannot be created, if decompression or
/// flushing fails, or if the output file cannot be synced.
fn decompress_catalog_snapshot(compressed_path: &Path, output_path: &Path) -> Result<()> {
    let compressed_file =
        File::open(compressed_path).context("failed to open compressed catalog snapshot")?;
    let mut decoder = Decoder::new(compressed_file)
        .context("failed to create zstd decoder for catalog snapshot")?;
    let output_file =
        File::create(output_path).context("failed to create catalog snapshot temp file")?;
    let mut writer = BufWriter::new(output_file);

    std::io::copy(&mut decoder, &mut writer).context("failed to decompress catalog snapshot")?;
    writer.flush().context("failed to flush catalog snapshot")?;

    let output_file = writer
        .into_inner()
        .map_err(|err| err.into_error())
        .context("failed to finalize catalog snapshot temp file")?;
    output_file
        .sync_all()
        .context("failed to sync catalog snapshot temp file")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::types::CatalogDownloadPlan;
    use super::{compressed_snapshot_temp_path, download_catalog_release};
    use crate::core::network::build_client;
    use crate::models::catalog::CatalogMetadata;
    use std::collections::BTreeMap;
    use tempfile::tempdir;
    use winbrew_testing::MockServer;

    #[test]
    fn download_catalog_release_removes_internal_compressed_temp_file_on_decompression_failure() {
        let temp_dir = tempdir().expect("temp dir");
        let catalog_temp_path = temp_dir.path().join("catalog.db");
        let metadata_temp_path = temp_dir.path().join("metadata.json");
        let client = build_client("winbrew-app-tests").expect("build client");
        let mut server = MockServer::new();

        let metadata = CatalogMetadata::build_from_counts(
            1,
            BTreeMap::from([(String::from("winget"), 1)]),
            String::from("sha256:expected"),
        );
        let metadata_url = format!("{}/metadata.json", server.url());
        let catalog_url = format!("{}/catalog.db.zst", server.url());

        let _metadata_mock = server.mock_get(
            "/metadata.json",
            serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
        );
        let _catalog_mock = server.mock_get("/catalog.db.zst", b"not valid zstd");

        let plan = CatalogDownloadPlan::Full {
            catalog_url,
            metadata_url,
            expected_hash: Some(String::from("sha256:expected")),
        };

        let result = download_catalog_release(
            &client,
            &plan,
            &catalog_temp_path,
            &metadata_temp_path,
            |_| {},
            |_| {},
        );

        let error = result.expect_err("expected decompression failure");

        assert!(
            error
                .to_string()
                .contains("failed to decompress catalog snapshot")
        );
        assert!(!compressed_snapshot_temp_path(&catalog_temp_path).exists());
        assert!(metadata_temp_path.exists());
    }
}
