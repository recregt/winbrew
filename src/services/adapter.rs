use anyhow::Result;
use rusqlite::Connection;

use crate::{manifest::Manifest, sources};

pub fn fetch_manifest(conn: &Connection, name: &str, version: &str) -> Result<Manifest> {
    let source = sources::active_source(conn)?;
    source.fetch_manifest(conn, name, version)
}
