pub mod catalog;
pub mod download;
pub mod state;
pub mod types;
pub mod workspace;

use anyhow::Result;
use std::fs;

use crate::core::fs::cleanup_path;
use crate::core::network::installer_filename;
use crate::database;
use crate::engines;
use crate::engines::PackageEngine;
use crate::models::CatalogPackage;

pub use types::InstallResult;

pub fn run<FChoose, FStart, FProgress>(
    query: &[String],
    mut choose_package: FChoose,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<InstallResult>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let query_text = query.join(" ").trim().to_owned();
    if query_text.is_empty() {
        anyhow::bail!("package query cannot be empty");
    }

    let catalog_conn = database::get_catalog_conn()?;
    let package = resolve_catalog_package(&catalog_conn, &query_text, &mut choose_package)?;
    let installer =
        catalog::select_installer(&database::get_installers(&catalog_conn, &package.id)?)?;
    let engine = engines::get_engine(&installer)?;

    let install_dir = database::Config::current()
        .resolved_paths()
        .packages
        .join(&package.name);
    let temp_root = workspace::build_temp_root(&package.name, &package.version);

    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&temp_root)?;

    let conn = database::get_conn()?;
    state::prepare_install_target(&conn, &package.name, &install_dir)?;
    state::mark_installing(
        &conn,
        package.name.clone(),
        package.version.clone(),
        installer.kind.clone(),
        &install_dir,
    )?;

    let client = download::build_client()?;

    let result = perform_install(
        &client,
        engine,
        &installer,
        &temp_root,
        &install_dir,
        on_start,
        on_progress,
    );

    match result {
        Ok(()) => {
            let install_result = InstallResult {
                name: package.name,
                version: package.version,
                install_dir: install_dir.to_string_lossy().to_string(),
            };

            state::mark_ok(&conn, &install_result.name)?;
            let _ = cleanup_path(&temp_root);
            Ok(install_result)
        }
        Err(err) => {
            let _ = state::mark_failed(&conn, &package.name);
            let _ = cleanup_path(&temp_root);
            Err(err)
        }
    }
}

fn perform_install<FStart, FProgress>(
    client: &reqwest::blocking::Client,
    engine: crate::engines::EngineKind,
    installer: &crate::models::CatalogInstaller,
    temp_root: &std::path::Path,
    install_dir: &std::path::Path,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let download_path = temp_root.join(installer_filename(&installer.url));
    download::download_installer(client, installer, &download_path, on_start, on_progress)?;

    engine.install(installer, &download_path, install_dir)?;

    Ok(())
}

fn resolve_catalog_package<FChoose>(
    conn: &rusqlite::Connection,
    query: &str,
    choose_package: &mut FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let matches = catalog::search_catalog_packages(conn, query)?;

    if matches.is_empty() {
        anyhow::bail!("no catalog packages matched '{query}'");
    }

    if matches.len() == 1 {
        return Ok(matches.into_iter().next().expect("single match exists"));
    }

    if let Some(exact_index) = matches
        .iter()
        .position(|pkg| pkg.name.eq_ignore_ascii_case(query))
    {
        return Ok(matches.into_iter().nth(exact_index).unwrap());
    }

    let selected = choose_package(query, &matches)?;

    matches
        .into_iter()
        .nth(selected)
        .ok_or_else(|| anyhow::anyhow!("selected package index was out of range"))
}
