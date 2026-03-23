use anyhow::{Context, Result, bail};
use rusqlite::Connection;

use crate::core::network::http;
use crate::database;
use crate::manifest::Manifest;
use crate::sources::SourceAdapter;

pub struct WingetSource;

impl SourceAdapter for WingetSource {
    fn fetch_manifest(&self, conn: &Connection, name: &str, version: &str) -> Result<Manifest> {
        let url = manifest_url(conn, name, version)?;
        let client = http::build_client(conn)?;
        let format = manifest_format(conn)?;

        let content = http::apply_github_auth(conn, &url, client.get(&url))?
            .send()
            .context("failed to connect")?
            .error_for_status()
            .context("manifest not found")?
            .text()
            .context("failed to read manifest")?;

        parse_manifest(&format, &content)
    }
}

pub(crate) fn manifest_url(conn: &Connection, name: &str, version: &str) -> Result<String> {
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

    let config = database::Config::current();
    Ok(config.sources.winget.url)
}

pub(crate) fn manifest_format(conn: &Connection) -> Result<String> {
    let _ = conn;

    let config = database::Config::current();
    Ok(config.sources.winget.format)
}

pub(crate) fn parse_manifest(format: &str, content: &str) -> Result<Manifest> {
    match format.trim().to_ascii_lowercase().as_str() {
        "toml" | "winget_toml" | "custom_toml" => Manifest::parse_toml(content),
        "yaml" | "yml" | "winget_yaml" => {
            bail!("yaml parsing is not wired yet; add a YAML manifest adapter for this source")
        }
        other => bail!("unsupported manifest format: {other}"),
    }
}
