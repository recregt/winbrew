use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use crate::models::CatalogInstaller;

const CATALOG_USER_AGENT: &str = "winbrew-package-installer";

pub fn download_installer<FStart, FProgress>(
    installer: &CatalogInstaller,
    download_path: &Path,
    on_start: FStart,
    mut on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let client = reqwest::blocking::Client::builder()
        .user_agent(CATALOG_USER_AGENT)
        .build()
        .context("failed to build HTTP client")?;

    let mut response = client
        .get(&installer.url)
        .send()
        .with_context(|| format!("failed to request installer {}", installer.url))?
        .error_for_status()
        .context("installer request failed")?;

    on_start(response.content_length());

    let mut file = fs::File::create(download_path).with_context(|| {
        format!(
            "failed to create installer download file at {}",
            download_path.display()
        )
    })?;

    let mut buffer = [0u8; 16 * 1024];
    let mut hasher = Sha256::new();

    loop {
        let read = response
            .read(&mut buffer)
            .context("failed to read installer response")?;
        if read == 0 {
            break;
        }

        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .context("failed to write installer to disk")?;
        on_progress(read as u64);
    }

    verify_hash(&installer.hash, hasher.finalize())?;
    Ok(())
}

pub fn installer_filename(url: &str) -> String {
    url.rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("download.bin")
        .to_string()
}

pub fn is_zip_path(url: &str) -> bool {
    installer_filename(url)
        .rsplit_once('.')
        .map(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn verify_hash(expected_hash: &str, actual_hash: impl AsRef<[u8]>) -> Result<()> {
    let expected_hash = normalize_hash(expected_hash);
    if expected_hash.is_empty() {
        return Ok(());
    }

    let actual_hash = actual_hash
        .as_ref()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();

    if actual_hash != expected_hash {
        bail!("checksum mismatch for installer: expected {expected_hash}, got {actual_hash}");
    }

    Ok(())
}

fn normalize_hash(value: &str) -> String {
    value
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(value.trim())
        .to_ascii_lowercase()
}
