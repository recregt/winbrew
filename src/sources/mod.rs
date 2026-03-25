use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::models::PackageCandidate;
use crate::{database, manifest::Manifest};

pub mod winget;

pub trait SourceAdapter {
    fn fetch_manifest(&self, conn: &Connection, name: &str, version: &str) -> Result<Manifest>;

    fn search_packages(&self, query: &str) -> Result<Vec<PackageCandidate>>;
}

pub fn active_source() -> Result<Box<dyn SourceAdapter + Send + Sync>> {
    resolve_source()
}

pub(crate) fn winget_registry_url() -> String {
    let config = database::Config::current();
    config
        .effective_value("sources.winget.url")
        .map(|(value, _)| value)
        .unwrap_or(config.sources.winget.url)
}

pub(crate) fn winget_repo_slug() -> Option<String> {
    let config = database::Config::current();

    if let Ok(Some((value, _))) = config.effective_optional_value("sources.winget.repo_slug") {
        let trimmed = value.trim();

        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let url = winget_registry_url();
    let trimmed = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut parts = trimmed.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    Some(format!("{owner}/{repo}"))
}

pub(crate) fn winget_api_base() -> String {
    let config = database::Config::current();
    config
        .effective_value("sources.winget.api_base")
        .map(|(value, _)| value)
        .unwrap_or(config.sources.winget.api_base)
}

pub(crate) fn winget_manifest_format() -> String {
    let config = database::Config::current();
    config
        .effective_value("sources.winget.format")
        .map(|(value, _)| value)
        .unwrap_or(config.sources.winget.format)
}

pub(crate) fn winget_manifest_kind() -> String {
    let config = database::Config::current();
    config
        .effective_value("sources.winget.manifest_kind")
        .map(|(value, _)| value)
        .unwrap_or(config.sources.winget.manifest_kind)
}

pub(crate) fn winget_manifest_path_template() -> String {
    let config = database::Config::current();
    config
        .effective_value("sources.winget.manifest_path_template")
        .map(|(value, _)| value)
        .unwrap_or(config.sources.winget.manifest_path_template)
}

fn resolve_source() -> Result<Box<dyn SourceAdapter + Send + Sync>> {
    let config = database::Config::current();

    match config.sources.primary.as_str() {
        "winget" => Ok(Box::new(winget::WingetSource)),
        other => bail!("unsupported source: {other}"),
    }
}
