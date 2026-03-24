use anyhow::{Context, Result};
use reqwest::{
    Proxy,
    blocking::{Client, RequestBuilder},
};
use std::time::Duration;
use tracing::{debug, trace};

use crate::database;

#[derive(Debug, Clone)]
pub struct NetworkSettings {
    pub timeout_secs: u64,
    pub proxy_url: Option<String>,
    pub github_token: Option<String>,
}

impl NetworkSettings {
    pub fn current() -> Self {
        let config = database::Config::current();

        Self {
            timeout_secs: config.core.download_timeout,
            proxy_url: config.core.proxy,
            github_token: config.core.github_token,
        }
    }
}

pub fn build_client_with(settings: &NetworkSettings) -> Result<Client> {
    let timeout_secs = settings.timeout_secs;

    debug!(timeout_secs, "building HTTP client");

    let mut builder = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("winbrew/0.1");

    if let Some(proxy_url) = &settings.proxy_url {
        trace!(proxy = proxy_url.as_str(), "configuring HTTP proxy");
        builder = builder.proxy(Proxy::all(proxy_url.as_str()).context("invalid proxy URL")?);
    }

    builder.build().context("failed to build HTTP client")
}

pub fn apply_github_auth_with(
    settings: &NetworkSettings,
    url: &str,
    request: RequestBuilder,
) -> Result<RequestBuilder> {
    if is_github_url(url)
        && let Some(token) = &settings.github_token
    {
        trace!(url = url, "applying GitHub authorization");
        return Ok(request.bearer_auth(token.clone()));
    }

    Ok(request)
}

fn is_github_url(url: &str) -> bool {
    url.contains("github.com")
        || url.contains("githubusercontent.com")
        || url.contains("api.github.com")
}
