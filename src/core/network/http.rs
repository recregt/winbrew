use anyhow::{Context, Result};
use reqwest::{
    Proxy,
    blocking::{Client, RequestBuilder},
};
use rusqlite::Connection;
use std::time::Duration;
use tracing::{debug, trace};

use crate::database;

const DEFAULT_DOWNLOAD_TIMEOUT_SECS: u64 = 30;

pub fn build_client(conn: &Connection) -> Result<Client> {
    let _ = conn;

    let timeout_secs =
        database::config_u64("download_timeout")?.unwrap_or(DEFAULT_DOWNLOAD_TIMEOUT_SECS);

    debug!(timeout_secs, "building HTTP client");

    let mut builder = Client::builder().timeout(Duration::from_secs(timeout_secs));

    if let Some(proxy_url) = config_value("proxy")? {
        trace!(proxy = proxy_url.as_str(), "configuring HTTP proxy");
        builder = builder.proxy(Proxy::all(&proxy_url).context("invalid proxy URL")?);
    }

    builder.build().context("failed to build HTTP client")
}

pub fn apply_github_auth(
    conn: &Connection,
    url: &str,
    request: RequestBuilder,
) -> Result<RequestBuilder> {
    let _ = conn;

    if is_github_url(url)
        && let Some(token) = config_value("github_token")?
    {
        trace!(url = url, "applying GitHub authorization");
        return Ok(request.bearer_auth(token));
    }

    Ok(request)
}

fn config_value(key: &str) -> Result<Option<String>> {
    database::config_string(key)
}

fn is_github_url(url: &str) -> bool {
    url.contains("github.com")
        || url.contains("githubusercontent.com")
        || url.contains("api.github.com")
}
