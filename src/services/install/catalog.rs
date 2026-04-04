use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::database;
use crate::models::{CatalogInstaller, CatalogPackage};

pub fn resolve_catalog_package(conn: &Connection, query: &str) -> Result<CatalogPackage> {
    let matches = database::search(conn, query)?;

    if matches.is_empty() {
        bail!("no catalog packages matched '{query}'");
    }

    if let Some(exact) = matches
        .iter()
        .find(|pkg| pkg.name.eq_ignore_ascii_case(query))
        .cloned()
    {
        return Ok(exact);
    }

    if matches.len() == 1 {
        return Ok(matches.into_iter().next().expect("single match exists"));
    }

    let candidates = matches
        .iter()
        .take(8)
        .map(|pkg| format!("{} ({})", pkg.name, pkg.id))
        .collect::<Vec<_>>()
        .join(", ");

    bail!("query '{query}' matched multiple packages; be more specific: {candidates}")
}

pub fn select_installer(installers: &[CatalogInstaller]) -> Result<CatalogInstaller> {
    let current_arch = current_arch_name();

    installers
        .iter()
        .find(|installer| installer.arch.eq_ignore_ascii_case(current_arch))
        .cloned()
        .or_else(|| {
            installers
                .iter()
                .find(|installer| installer.arch.trim().is_empty())
                .cloned()
        })
        .or_else(|| installers.first().cloned())
        .ok_or_else(|| anyhow::anyhow!("catalog package has no installers"))
}

fn current_arch_name() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    }
}
