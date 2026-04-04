use anyhow::{Context, Result};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use crate::core::paths;

const CATALOG_DIRECT_DOWNLOAD_URL: &str =
    "https://github.com/recregt/winbrew/releases/latest/download/catalog.db";

pub fn refresh_catalog<FStart, FProgress>(on_start: FStart, on_progress: FProgress) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let catalog_path = paths::catalog_db();
    let catalog_dir = catalog_path
        .parent()
        .context("failed to resolve catalog database directory")?;

    fs::create_dir_all(catalog_dir).context("failed to create catalog database directory")?;

    let temp_path = catalog_dir.join("catalog.db.download");
    if temp_path.exists() {
        fs::remove_file(&temp_path).context("failed to clear previous catalog download")?;
    }

    download_catalog_release(&temp_path, on_start, on_progress)?;

    fs::rename(&temp_path, &catalog_path).with_context(|| {
        format!(
            "failed to move downloaded catalog into place: {} -> {}",
            temp_path.display(),
            catalog_path.display()
        )
    })?;

    Ok(())
}

fn download_catalog_release<FStart, FProgress>(
    temp_path: &Path,
    on_start: FStart,
    mut on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let client = reqwest::blocking::Client::builder()
        .user_agent("winbrew-catalog-downloader")
        .build()
        .context("failed to build HTTP client")?;

    let mut response = client
        .get(CATALOG_DIRECT_DOWNLOAD_URL)
        .send()
        .context("failed to request catalog asset")?
        .error_for_status()
        .context("catalog asset request failed")?;

    let total_bytes = response.content_length();
    on_start(total_bytes);

    let mut file = fs::File::create(temp_path)
        .with_context(|| format!("failed to create download file at {}", temp_path.display()))?;

    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .context("failed to read catalog asset")?;
        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read])
            .context("failed to write catalog asset to disk")?;
        on_progress(read as u64);
    }

    Ok(())
}
