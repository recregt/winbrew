use anyhow::{Result, bail};
use std::path::Path;

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
        catalog_temp_path,
        "catalog asset",
        on_start,
        on_progress,
        |_| Ok(()),
    )?;

    verify_catalog_hash(catalog_temp_path, &metadata.current_hash)?;

    Ok(())
}
