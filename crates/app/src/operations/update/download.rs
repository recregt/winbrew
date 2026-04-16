use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use zstd::stream::read::Decoder;

use crate::core::network::{Client, download_url_to_temp_file};

use super::metadata::{load_catalog_metadata, verify_catalog_hash};
use super::types::CatalogDownloadPlan;

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
