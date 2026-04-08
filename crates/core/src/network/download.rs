use anyhow::{Context, Result};
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

pub type Client = reqwest::blocking::Client;

pub fn build_client(user_agent: &str) -> Result<Client> {
    Client::builder()
        .user_agent(user_agent)
        .build()
        .context("failed to build HTTP client")
}

pub fn download_url_to_temp_file<FStart, FProgress, FChunk>(
    client: &Client,
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

pub fn installer_filename(url: &str) -> String {
    url_path(url)
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("download.bin")
        .to_string()
}

pub fn is_zip_path(url: &str) -> bool {
    let path = url_path(url);

    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
}

fn url_path(url: &str) -> &str {
    let path = url.split('#').next().unwrap_or(url);
    path.split('?').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{installer_filename, is_zip_path};

    #[test]
    fn installer_filename_uses_last_segment() {
        assert_eq!(
            installer_filename("https://example.invalid/a/b/tool.zip"),
            "tool.zip"
        );
    }

    #[test]
    fn installer_filename_ignores_query_and_fragment() {
        assert_eq!(
            installer_filename("https://example.invalid/tool.exe?token=123#xyz"),
            "tool.exe"
        );
    }

    #[test]
    fn is_zip_path_ignores_query_string() {
        assert!(is_zip_path("https://example.invalid/tool.zip?token=abc"));
        assert!(!is_zip_path("https://example.invalid/tool.exe?token=abc"));
    }
}
