use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use rusqlite::Connection;

use crate::core::{fs::DownloadTarget, hash};

use crate::core::network::http;

const BUFFER_SIZE: usize = 64 * 1024;

pub fn send_request(
    conn: &Connection,
    url: &str,
    dest: &Path,
) -> Result<reqwest::blocking::Response> {
    let client = http::build_client(conn)?;
    let requested_existing_size = existing_part_size(dest);

    let mut request = http::apply_github_auth(conn, url, client.get(url))?;
    if requested_existing_size > 0 {
        request = request.header("Range", format!("bytes={}-", requested_existing_size));
    }

    request
        .send()
        .context("failed to connect")?
        .error_for_status()
        .context("server returned error")
}

pub fn open_target(dest: &Path, response: &reqwest::blocking::Response) -> Result<DownloadTarget> {
    let requested_existing_size = existing_part_size(dest);
    DownloadTarget::new(dest, response, requested_existing_size)
}

fn existing_part_size(dest: &Path) -> u64 {
    let temp_dest = dest.with_extension("part");

    if temp_dest.exists() {
        fs::metadata(&temp_dest).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    }
}

pub fn stream_response<F>(
    response: &mut reqwest::blocking::Response,
    target: &mut DownloadTarget,
    expected_checksum: Option<&str>,
    on_progress: &mut F,
) -> Result<Option<Sha256>>
where
    F: FnMut(u64, u64),
{
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut downloaded = target.existing_size;
    let mut hasher = init_hasher(expected_checksum, &target.temp_path, target.existing_size)?;

    loop {
        let read = response
            .read(&mut buffer)
            .context("failed to read response")?;

        if read == 0 {
            break;
        }

        target
            .writer
            .write_all(&buffer[..read])
            .context("failed to write file")?;

        if let Some(hasher) = hasher.as_mut() {
            hasher.update(&buffer[..read]);
        }

        downloaded += read as u64;
        on_progress(downloaded, target.total_size);
    }

    if target.total_size > 0 && downloaded != target.total_size {
        bail!(
            "incomplete download: expected {} bytes, got {}",
            target.total_size,
            downloaded
        );
    }

    Ok(hasher)
}

pub fn verify_download(
    target: &DownloadTarget,
    hasher: Option<Sha256>,
    expected_checksum: Option<&str>,
) -> Result<()> {
    if let Some(expected) = expected_checksum {
        let expected = expected.strip_prefix("sha256:").unwrap_or(expected);
        let actual = hex::encode(
            hasher
                .expect("checksum hasher must exist when verification is requested")
                .finalize(),
        );

        if actual != expected {
            let _ = fs::remove_file(&target.temp_path);
            return Err(anyhow!(
                "checksum mismatch\n  expected: {}\n  actual:   {}",
                expected,
                actual
            ));
        }
    }

    Ok(())
}

fn init_hasher(
    expected_checksum: Option<&str>,
    temp_path: &Path,
    existing_size: u64,
) -> Result<Option<Sha256>> {
    let mut hasher = expected_checksum.map(|_| Sha256::new());

    if existing_size > 0
        && let Some(hasher) = hasher.as_mut()
    {
        hash::seed_hasher(temp_path, hasher)?;
    }

    Ok(hasher)
}
