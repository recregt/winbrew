use anyhow::{Context, Result};
use reqwest::{
    Proxy,
    blocking::{Client, RequestBuilder},
};
use std::time::Duration;
use tracing::{debug, trace};

use crate::database;

pub fn build_client() -> Result<Client> {
    let config = database::Config::current();
    let timeout_secs = config.core.download_timeout;

    debug!(timeout_secs, "building HTTP client");

    let mut builder = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("winbrew/0.1");

    if let Some(proxy_url) = config.core.proxy {
        trace!(proxy = proxy_url.as_str(), "configuring HTTP proxy");
        builder = builder.proxy(Proxy::all(&proxy_url).context("invalid proxy URL")?);
    }

    builder.build().context("failed to build HTTP client")
}

pub fn apply_github_auth(url: &str, request: RequestBuilder) -> Result<RequestBuilder> {
    let config = database::Config::current();

    if is_github_url(url)
        && let Some(token) = config.core.github_token
    {
        trace!(url = url, "applying GitHub authorization");
        return Ok(request.bearer_auth(token));
    }

    Ok(request)
}

fn is_github_url(url: &str) -> bool {
    url.contains("github.com")
        || url.contains("githubusercontent.com")
        || url.contains("api.github.com")
}
