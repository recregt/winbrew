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
        let timeout_secs = config
            .effective_value("core.download_timeout")
            .ok()
            .and_then(|(value, _)| value.parse::<u64>().ok())
            .unwrap_or(config.core.download_timeout);
        let proxy_url = config
            .effective_value("core.proxy")
            .ok()
            .and_then(|(value, _)| if value.is_empty() { None } else { Some(value) })
            .or_else(|| config.core.proxy.clone());
        let github_token = config
            .effective_value("core.github_token")
            .ok()
            .and_then(|(value, _)| if value.is_empty() { None } else { Some(value) })
            .or_else(|| config.core.github_token.clone());

        Self {
            timeout_secs,
            proxy_url,
            github_token,
        }
    }
}

pub fn build_client_with(settings: &NetworkSettings) -> Result<Client> {
    let timeout_secs = settings.timeout_secs;

    debug!(timeout_secs, "building HTTP client");

    let user_agent = format!("winbrew/{}", env!("CARGO_PKG_VERSION"));
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(user_agent);

    if let Some(proxy_url) = &settings.proxy_url {
        trace!(proxy = %proxy_url, "configuring HTTP proxy");
        builder = builder.proxy(Proxy::all(proxy_url.as_str()).context("invalid proxy URL")?);
    }

    builder.build().context("failed to build HTTP client")
}

pub fn apply_github_auth_with(
    settings: &NetworkSettings,
    url: &str,
    request: RequestBuilder,
) -> Result<RequestBuilder> {
    if is_github_domain(url)
        && let Some(token) = &settings.github_token
    {
        trace!(url = url, "applying GitHub authorization");
        return Ok(request.bearer_auth(token.clone()));
    }

    Ok(request)
}

fn is_github_domain(url: &str) -> bool {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()));

    let Some(host) = host.as_deref() else {
        return false;
    };

    host == "github.com"
        || host == "api.github.com"
        || host == "raw.githubusercontent.com"
        || host.ends_with(".github.com")
        || host.ends_with(".githubusercontent.com")
}
