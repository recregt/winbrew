use anyhow::{Context, Result};
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

pub fn build_client(user_agent: &str) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(user_agent)
        .build()
        .context("failed to build HTTP client")
}

pub fn download_url_to_temp_file<FStart, FProgress, FChunk>(
    client: &reqwest::blocking::Client,
    url: &str,
    temp_path: &Path,
    label: &str,
    on_start: FStart,
    mut on_progress: FProgress,
    mut on_chunk: FChunk,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
    FChunk: FnMut(&[u8]) -> Result<()>,
{
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to request {label} {url}"))?
        .error_for_status()
        .with_context(|| format!("{label} request failed"))?;

    on_start(response.content_length());

    let file = fs::File::create(temp_path).with_context(|| {
        format!(
            "failed to create {label} download file at {}",
            temp_path.display()
        )
    })?;

    let mut writer = BufWriter::with_capacity(64 * 1024, file);
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let read = response
            .read(&mut buffer)
            .with_context(|| format!("failed to read {label}"))?;
        if read == 0 {
            break;
        }

        let chunk = &buffer[..read];
        on_chunk(chunk)?;
        writer
            .write_all(chunk)
            .with_context(|| format!("failed to write {label} to disk"))?;
        on_progress(read as u64);
    }

    writer
        .flush()
        .with_context(|| format!("failed to flush {label} download file"))?;

    Ok(())
}
