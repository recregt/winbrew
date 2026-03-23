use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::{database, manifest::Manifest};

pub mod winget;

pub trait SourceAdapter {
    fn fetch_manifest(&self, conn: &Connection, name: &str, version: &str) -> Result<Manifest>;
}

pub fn active_source(conn: &Connection) -> Result<Box<dyn SourceAdapter + Send + Sync>> {
    resolve_source(conn)
}

pub(crate) fn winget_registry_url() -> String {
    database::Config::current().sources.winget.url
}

pub(crate) fn winget_manifest_format() -> String {
    database::Config::current().sources.winget.format
}

pub(crate) fn winget_manifest_kind() -> String {
    database::Config::current()
        .sources
        .winget
        .manifest_kind
        .clone()
}

pub(crate) fn winget_manifest_path_template() -> String {
    database::Config::current()
        .sources
        .winget
        .manifest_path_template
        .clone()
}

fn resolve_source(conn: &Connection) -> Result<Box<dyn SourceAdapter + Send + Sync>> {
    let _ = conn;

    let config = database::Config::current();

    match config.sources.primary.as_str() {
        "winget" => Ok(Box::new(winget::WingetSource)),
        other => bail!("unsupported source: {other}"),
    }
}
