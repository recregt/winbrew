use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::{database, manifest::Manifest};

use super::http;

const DEFAULT_REGISTRY_URL: &str = "https://raw.githubusercontent.com/recregt/winbrew-pkgs/main";

pub fn fetch_manifest(conn: &Connection, name: &str, version: &str) -> Result<Manifest> {
    let url = manifest_url(conn, name, version)?;
    let client = http::build_client(conn)?;

    let content = http::apply_github_auth(conn, &url, client.get(&url))?
        .send()
        .context("failed to connect")?
        .error_for_status()
        .context("manifest not found")?
        .text()
        .context("failed to read manifest")?;

    Manifest::parse(&content)
}

fn manifest_url(conn: &Connection, name: &str, version: &str) -> Result<String> {
    let registry = registry_url(conn)?;
    Ok(format!(
        "{}/{}/{}.toml",
        registry.trim_end_matches('/'),
        name,
        version
    ))
}

fn registry_url(conn: &Connection) -> Result<String> {
    let _ = conn;

    match database::config_string("registry_url")? {
        Some(value) => Ok(value),
        None => Ok(DEFAULT_REGISTRY_URL.to_string()),
    }
}
