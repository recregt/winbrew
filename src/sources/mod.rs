use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::{database, manifest::Manifest};

pub mod winget;

pub trait SourceAdapter {
    fn fetch_manifest(&self, conn: &Connection, name: &str, version: &str) -> Result<Manifest>;
}

pub fn active_source(conn: &Connection) -> Result<Box<dyn SourceAdapter + Send + Sync>> {
    let _ = conn;

    let config = database::Config::current();

    match config.sources.primary.as_str() {
        "winget" => Ok(Box::new(winget::WingetSource)),
        other => bail!("unsupported source: {other}"),
    }
}
