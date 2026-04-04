use anyhow::Result;
use rusqlite::Connection;

use crate::database;
use crate::models::{CatalogInstaller, CatalogPackage};

pub fn search_catalog_packages(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    // Catalog search entry point for the install service.
    // Currently delegates to the database layer directly; this is where
    // result ranking, normalization, or exact-match priority will live.
    database::search(conn, query)
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
