pub mod catalog;
pub mod download;
pub mod flow;
pub mod recovery;
pub mod state;
pub mod types;
pub mod workspace;

use anyhow::Result;
use std::fs;

use crate::AppContext;
use crate::core::cancel;
use crate::database;
use crate::engines::{self, EngineKind};
use crate::models::CatalogPackage;

pub use types::{InstallOutcome, InstallResult};

pub fn run<FChoose, FStart, FProgress>(
    ctx: &AppContext,
    query: &[String],
    ignore_checksum_security: bool,
    mut choose_package: FChoose,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<InstallOutcome>
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

    let install_dir = ctx.paths.packages.join(&package.name);
    let temp_root = workspace::build_temp_root(&package.name, &package.version);

    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&temp_root)?;

    let conn = match database::get_conn() {
        Ok(conn) => conn,
        Err(err) => {
            flow::cleanup_temp_root(&temp_root);
            return Err(err);
        }
    };

    if let Err(err) = state::prepare_install_target(&conn, &package.name, &install_dir) {
        flow::cleanup_temp_root(&temp_root);
        return Err(err.into());
    }

    if let Err(err) = state::mark_installing(
        &conn,
        package.name.clone(),
        package.version.clone(),
        installer.kind.clone(),
        &install_dir,
    ) {
        flow::cleanup_temp_root(&temp_root);
        return Err(err.into());
    }

    let client = download::build_client()?;

    let legacy_checksum_algorithms = match flow::perform_install(flow::InstallRequest {
        client: &client,
        engine,
        installer: &installer,
        temp_root: &temp_root,
        install_dir: &install_dir,
        ignore_checksum_security,
        on_start,
        on_progress,
    }) {
        Ok(legacy_checksum_algorithms) => legacy_checksum_algorithms,
        Err(err) => {
            if flow::is_cancelled_error(&err) {
                flow::rollback_cancelled_install(&conn, &package.name, &install_dir, &temp_root);
            } else {
                flow::rollback_failed_install(&conn, &package.name, &install_dir, &temp_root);
            }
            return Err(err);
        }
    };

    if cancel::is_cancelled() {
        flow::rollback_cancelled_install(&conn, &package.name, &install_dir, &temp_root);
        return Err(cancel::CancellationError.into());
    }

    let install_result = InstallResult {
        name: package.name,
        version: package.version,
        install_dir: install_dir.to_string_lossy().to_string(),
    };

    let msix_package_full_name = if engine == EngineKind::Msix {
        match engines::msix::installed_package_full_name(&install_result.name) {
            Ok(full_name) => Some(full_name),
            Err(err) => {
                flow::rollback_failed_install(
                    &conn,
                    &install_result.name,
                    &install_dir,
                    &temp_root,
                );
                return Err(err);
            }
        }
    } else {
        None
    };

    if let Err(err) = state::mark_ok(
        &conn,
        &install_result.name,
        msix_package_full_name.as_deref(),
    ) {
        let _ = state::mark_failed(&conn, &install_result.name);
        flow::cleanup_temp_root(&temp_root);
        return Err(err.into());
    }

    flow::cleanup_temp_root(&temp_root);

    Ok(InstallOutcome {
        result: install_result,
        legacy_checksum_algorithms,
    })
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
