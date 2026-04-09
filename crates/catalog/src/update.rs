use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::core::fs::finalize_temp_file;
use crate::core::network::{build_client, download_url_to_temp_file};
use crate::core::paths::ResolvedPaths;

const CATALOG_DIRECT_DOWNLOAD_URL: &str =
    "https://github.com/recregt/winbrew/releases/latest/download/catalog.db";

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

    let temp_path = catalog_dir.join("catalog.db.download");
    if temp_path.exists() {
        fs::remove_file(&temp_path).context("failed to clear previous catalog download")?;
    }

    download_catalog_release(&temp_path, on_start, on_progress)?;

    finalize_temp_file(&temp_path, &catalog_path)?;

    Ok(())
}

fn download_catalog_release<FStart, FProgress>(
    temp_path: &Path,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let client = build_client("winbrew-catalog-downloader")?;

    Ok(download_url_to_temp_file(
        &client,
        CATALOG_DIRECT_DOWNLOAD_URL,
        temp_path,
        "catalog asset",
        on_start,
        on_progress,
        |_| Ok(()),
    )?)
}
