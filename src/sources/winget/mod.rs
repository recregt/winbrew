use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::core::network::http;
use crate::manifest::Manifest;
use crate::models::PackageCandidate;
use crate::sources::SourceAdapter;

mod manifest;
mod search;

use manifest::{manifest_format, manifest_url, parse_manifest};

pub struct WingetSource;

impl SourceAdapter for WingetSource {
    fn fetch_manifest(&self, conn: &Connection, name: &str, version: &str) -> Result<Manifest> {
        let url = manifest_url(conn, name, version)?;
        let client = http::build_client()?;
        let format = manifest_format(conn)?;

        let content = http::apply_github_auth(&url, client.get(&url))?
            .send()
            .context("failed to connect")?
            .error_for_status()
            .context("manifest not found")?
            .text()
            .context("failed to read manifest")?;

        parse_manifest(&format, &content)
    }

    fn search_packages(&self, query: &str) -> Result<Vec<PackageCandidate>> {
        search::search_packages(query)
    }
}
