use anyhow::{Context, Result};
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::time::Duration;

/// Blocking HTTP client used by the download helpers.
pub type Client = reqwest::blocking::Client;

const DOWNLOAD_REQUEST_TIMEOUT_SECS: u64 = 300;
const DOWNLOAD_CONNECT_TIMEOUT_SECS: u64 = 30;
const DOWNLOAD_READ_BUFFER_SIZE: usize = 256 * 1024;
const DOWNLOAD_WRITE_BUFFER_SIZE: usize = 1024 * 1024;
const PROGRESS_REPORT_INTERVAL: u64 = 1024 * 1024;

/// Builds the shared blocking HTTP client for downloads.
///
/// The client applies the caller-provided user agent plus the shared request and
/// connect timeouts used across the download pipeline.
pub fn build_client(user_agent: &str) -> Result<Client> {
    Client::builder()
        .user_agent(user_agent)
        .timeout(Duration::from_secs(DOWNLOAD_REQUEST_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(DOWNLOAD_CONNECT_TIMEOUT_SECS))
        .build()
        .context("failed to build HTTP client")
}

/// Streams `url` into `temp_path`.
///
/// `on_start` receives the server-reported content length when available.
/// `on_progress` receives byte deltas since the previous progress callback, which
/// matches the existing `inc(...)`-style callers in the UI layer.
/// `on_chunk` sees each raw chunk before it is written to disk.
/// If the server reports a content length, the helper verifies the streamed byte
/// count matches it before committing the temp file.
/// The temporary file is removed automatically if streaming or finalization fails.
pub fn download_url_to_temp_file<FStart, FProgress, FChunk>(
    client: &Client,
    url: &str,
    temp_path: &Path,
    label: impl std::fmt::Display,
    on_start: FStart,
    mut on_progress: FProgress,
    mut on_chunk: FChunk,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
    FChunk: FnMut(&[u8]) -> Result<()>,
{
    let label = label.to_string();
    let mut temp_file_guard = TempFileGuard::new(temp_path);

    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to request {label} {url}"))?
        .error_for_status()
        .with_context(|| format!("{label} request failed"))?;

    let content_length = response.content_length();

    let file = fs::File::create(temp_path).with_context(|| {
        format!(
            "failed to create {label} download file at {}",
            temp_path.display()
        )
    })?;

    if let Some(total_size) = content_length {
        file.set_len(total_size)
            .with_context(|| format!("failed to pre-allocate {label} download file"))?;
        // `set_len` reserves the size, but the cursor still starts at byte 0.
    }

    on_start(content_length);

    let mut writer = BufWriter::with_capacity(DOWNLOAD_WRITE_BUFFER_SIZE, file);
    let mut buffer = [0u8; DOWNLOAD_READ_BUFFER_SIZE];
    let mut downloaded: u64 = 0;
    let mut last_reported: u64 = 0;

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

        downloaded += read as u64;
        if downloaded - last_reported >= PROGRESS_REPORT_INTERVAL {
            on_progress(downloaded - last_reported);
            last_reported = downloaded;
        }
    }

    if last_reported != downloaded {
        on_progress(downloaded - last_reported);
    }

    validate_download_size(&label, content_length, downloaded)?;

    let file = writer
        .into_inner()
        .map_err(|err| err.into_error())
        .with_context(|| format!("failed to finalize {label} download buffer"))?;

    // This is the durability boundary for the temp file; callers only rename it.
    file.sync_all()
        .with_context(|| format!("failed to sync {label} download file"))?;

    temp_file_guard.commit();

    Ok(())
}

/// Returns the last URL path segment or `download.bin` when the URL does not expose one.
pub fn installer_filename(url: &str) -> String {
    last_path_segment(url).unwrap_or_else(|| "download.bin".to_string())
}

/// Returns `true` when the URL path ends in `.zip`, ignoring query and fragment parts.
pub fn is_zip_path(url: &str) -> bool {
    last_path_segment(url).is_some_and(|segment| {
        segment
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
    })
}

fn last_path_segment(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;

    parsed
        .path_segments()?
        .next_back()
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
}

fn validate_download_size(label: &str, expected: Option<u64>, actual: u64) -> Result<()> {
    if let Some(expected) = expected
        && actual != expected
    {
        return Err(anyhow::anyhow!(
            "{label} size mismatch: expected {expected}, got {actual}"
        ));
    }

    Ok(())
}

struct TempFileGuard<'a> {
    path: &'a Path,
    committed: bool,
}

impl<'a> TempFileGuard<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for TempFileGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(self.path);
        }
    }
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
    fn installer_filename_falls_back_when_last_segment_is_empty() {
        assert_eq!(
            installer_filename("https://example.invalid/downloads/"),
            "download.bin"
        );
    }

    #[test]
    fn is_zip_path_ignores_query_string() {
        assert!(is_zip_path("https://example.invalid/tool.zip?token=abc"));
        assert!(!is_zip_path("https://example.invalid/tool.exe?token=abc"));
    }

    #[test]
    fn is_zip_path_rejects_empty_last_segment() {
        assert!(!is_zip_path("https://example.invalid/downloads/"));
    }

    #[test]
    fn is_zip_path_is_case_insensitive() {
        assert!(is_zip_path("https://example.invalid/tool.ZIP"));
        assert!(is_zip_path("https://example.invalid/tool.Zip"));
    }

    #[test]
    fn validate_download_size_accepts_matching_length() {
        assert!(super::validate_download_size("installer", Some(42), 42).is_ok());
    }

    #[test]
    fn validate_download_size_rejects_length_mismatch() {
        let error = super::validate_download_size("installer", Some(42), 41)
            .expect_err("expected size mismatch error");

        assert!(
            error
                .to_string()
                .contains("installer size mismatch: expected 42, got 41")
        );
    }
}
