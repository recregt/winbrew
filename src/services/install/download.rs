use anyhow::{Context, Result};
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use crate::core::hash::{sha256_finalize, sha256_hasher, sha256_update, verify_hash};
use crate::models::CatalogInstaller;

const CATALOG_USER_AGENT: &str = "winbrew-package-installer";

pub fn build_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(CATALOG_USER_AGENT)
        .build()
        .context("failed to build HTTP client")
}

pub fn download_installer<FStart, FProgress>(
    client: &reqwest::blocking::Client,
    installer: &CatalogInstaller,
    download_path: &Path,
    on_start: FStart,
    mut on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let temp_path = download_path.with_extension("part");
    let result = (|| -> Result<()> {
        let mut response = client
            .get(&installer.url)
            .send()
            .with_context(|| format!("failed to request installer {}", installer.url))?
            .error_for_status()
            .context("installer request failed")?;

        on_start(response.content_length());

        let file = fs::File::create(&temp_path).with_context(|| {
            format!(
                "failed to create installer download file at {}",
                temp_path.display()
            )
        })?;

        let mut writer = BufWriter::with_capacity(64 * 1024, file);
        let mut buffer = [0u8; 64 * 1024];
        let mut hasher = sha256_hasher();

        loop {
            let read = response
                .read(&mut buffer)
                .context("failed to read installer response")?;
            if read == 0 {
                break;
            }

            let chunk = &buffer[..read];
            sha256_update(&mut hasher, chunk);
            writer
                .write_all(chunk)
                .context("failed to write installer to disk")?;
            on_progress(read as u64);
        }

        writer
            .flush()
            .context("failed to flush installer download file")?;

        verify_hash(&installer.hash, sha256_finalize(hasher))?;

        if download_path.exists() {
            fs::remove_file(download_path).with_context(|| {
                format!(
                    "failed to remove stale installer file at {}",
                    download_path.display()
                )
            })?;
        }

        fs::rename(&temp_path, download_path).with_context(|| {
            format!(
                "failed to finalize installer file: {} -> {}",
                temp_path.display(),
                download_path.display()
            )
        })?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}
