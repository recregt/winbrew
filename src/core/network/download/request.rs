use anyhow::{Context, Result};
use tracing::{debug, trace};

use std::fs;
use std::path::Path;

use crate::core::fs::DownloadTarget;
use crate::core::network::http::{self, NetworkSettings};

pub fn send_request(
    settings: &NetworkSettings,
    url: &str,
    dest: &Path,
) -> Result<reqwest::blocking::Response> {
    send_request_inner(settings, url, dest, true)
}

fn send_request_inner(
    settings: &NetworkSettings,
    url: &str,
    dest: &Path,
    allow_resume: bool,
) -> Result<reqwest::blocking::Response> {
    debug!(url = url, destination = %dest.display(), "starting download request");

    let client = http::build_client_with(settings)?;
    let requested_existing_size = if allow_resume {
        existing_part_size(dest)
    } else {
        0
    };

    let mut request = http::apply_github_auth_with(settings, url, client.get(url))?;
    if requested_existing_size > 0 {
        request = request.header("Range", format!("bytes={}-", requested_existing_size));
    }

    let response = request.send().context("failed to connect")?;
    trace!(
        url = url,
        status = %response.status(),
        content_length = ?response.headers().get(reqwest::header::CONTENT_LENGTH),
        content_range = ?response.headers().get(reqwest::header::CONTENT_RANGE),
        "received HTTP response"
    );

    if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE
        && requested_existing_size > 0
    {
        debug!(
            url = url,
            destination = %dest.display(),
            "range not satisfiable; purging stale partial download and restarting"
        );

        let _ = fs::remove_file(dest.with_extension("part"));
        return send_request_inner(settings, url, dest, false);
    }

    response.error_for_status().context("server returned error")
}

pub fn open_target(dest: &Path, response: &reqwest::blocking::Response) -> Result<DownloadTarget> {
    let requested_existing_size = existing_part_size(dest);

    trace!(
        destination = %dest.display(),
        existing_size = requested_existing_size,
        content_length = ?response.headers().get(reqwest::header::CONTENT_LENGTH),
        "opening download target"
    );

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
