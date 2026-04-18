//! Catalog refresh workflow for the CLI.
//!
//! # Overview
//!
//! The refresh workflow is API-driven and runs in four phases:
//!
//! 1. Preparation: ensure the catalog directories exist and clear stale temp files.
//! 2. Selection: load local metadata if present, query the update API, and turn the
//!    response into a `current`, `patch`, or `full` plan.
//! 3. Execution: current plans return immediately, full plans download and verify a
//!    full snapshot, and patch plans apply incremental SQL patches to a working copy.
//! 4. Finalization: atomically rename the refreshed temp files into place and clean
//!    up any leftover temporary artifacts.
//!
//! # API Surface
//!
//! - `refresh_catalog` is the production entry point used by the CLI and targets the
//!   default update API.
//! - `refresh_catalog_with_api_url` is a doc-hidden test hook that lets integration
//!   tests point the workflow at a mock server.
//! - `api` builds safe update URLs and fetches the update selection payload.
//! - `planner` converts the selection response plus local metadata into a concrete
//!   `CatalogDownloadPlan`.
//! - `download` handles the full snapshot download, decompression, and final hash
//!   verification.
//! - `patch` applies incremental SQL patches against a working copy and writes the
//!   refreshed metadata.
//! - `metadata` loads local metadata, derives metadata URLs, and validates hashes.
//!
//! # Fallback Behavior
//!
//! If a patch application fails, the workflow clears the temp files, re-queries the
//! API for a full snapshot plan, and retries through the snapshot path.
//!
//! # Cleanup
//!
//! Temporary files are removed on both success and failure. The final catalog and
//! metadata files are only replaced after the new versions have been fully built.
//!
//! # Concurrency
//!
//! This module does not take a file lock. If multiple CLI processes can target the
//! same catalog root concurrently, that lock belongs at a higher layer.

mod api;
mod download;
mod metadata;
mod patch;
mod planner;
mod types;

use anyhow::{Context, Result, bail};
use std::path::Path;

use self::types::CatalogDownloadPlan;

use crate::core::fs::{cleanup_path, finalize_temp_file};
use crate::core::network::{Client, build_client};
use crate::core::paths::{ResolvedPaths, ensure_dirs_at};

const CATALOG_UPDATE_API_URL: &str = "https://api.winbrew.dev/v1/update";

/// Refreshes the local catalog using the default update API endpoint.
///
/// This is the production entry point used by the CLI. It delegates to the
/// injected-URL helper so tests can exercise the same workflow against a mock
/// server.
pub fn refresh_catalog<FStart, FProgress>(
    paths: &ResolvedPaths,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    refresh_catalog_with_api_url(paths, CATALOG_UPDATE_API_URL, on_start, on_progress)
}

/// Refreshes the local catalog using a caller-provided update API URL.
///
/// This is the same workflow as [`refresh_catalog`], but it keeps the API URL
/// injectable so integration tests can point the refresh logic at a mock
/// server. The function stays public for that reason, but it is hidden from the
/// generated API docs because it is not part of the intended CLI surface.
#[doc(hidden)]
pub fn refresh_catalog_with_api_url<FStart, FProgress>(
    paths: &ResolvedPaths,
    update_api_url: &str,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let catalog_path = paths.catalog_db.clone();
    let catalog_dir = catalog_path
        .parent()
        .context("failed to resolve catalog database directory")?;

    ensure_dirs_at(&paths.root).context("failed to create catalog directories")?;

    let catalog_temp_path = catalog_dir.join("catalog.db.download");
    let metadata_temp_path = catalog_dir.join("metadata.json.download");
    let metadata_path = catalog_dir.join("metadata.json");

    let result = (|| -> Result<()> {
        clear_temp_file(&catalog_temp_path)?;
        clear_temp_file(&metadata_temp_path)?;

        let client = build_client("winbrew-catalog-downloader")?;
        let local_metadata = metadata::load_local_catalog_metadata(&metadata_path)?;

        let selection =
            api::fetch_catalog_update_selection(&client, update_api_url, local_metadata.as_ref())?;
        let download_plan =
            match planner::plan_catalog_download(local_metadata.as_ref(), selection)? {
                Some(plan) => plan,
                None => request_full_snapshot_plan(&client, update_api_url)?,
            };

        match &download_plan {
            CatalogDownloadPlan::Current {
                current_hash,
                target_hash,
            } => {
                if current_hash != target_hash {
                    tracing::warn!(current_hash = %current_hash, target_hash = %target_hash, "update worker reported a current plan with mismatched hashes");
                }

                return Ok(());
            }
            CatalogDownloadPlan::Full { .. } => {
                download::download_catalog_release(
                    &client,
                    &download_plan,
                    &catalog_temp_path,
                    &metadata_temp_path,
                    on_start,
                    on_progress,
                )?;
            }
            CatalogDownloadPlan::Patch {
                patch_urls,
                expected_hash,
            } => {
                let previous_metadata = local_metadata
                    .as_ref()
                    .context("patch updates require local catalog metadata")?;

                if let Err(err) = patch::apply_catalog_patch_release(
                    &client,
                    &catalog_path,
                    &catalog_temp_path,
                    &metadata_temp_path,
                    patch_urls,
                    expected_hash,
                    previous_metadata.current_hash.as_str(),
                ) {
                    tracing::warn!(error = %err, "patch catalog update failed; falling back to full snapshot");
                    clear_temp_file(&catalog_temp_path)?;
                    clear_temp_file(&metadata_temp_path)?;

                    let fallback_plan = request_full_snapshot_plan(&client, update_api_url)?;
                    download::download_catalog_release(
                        &client,
                        &fallback_plan,
                        &catalog_temp_path,
                        &metadata_temp_path,
                        on_start,
                        on_progress,
                    )?;
                }
            }
        }

        finalize_temp_file(&catalog_temp_path, &catalog_path)?;
        finalize_temp_file(&metadata_temp_path, &metadata_path)?;

        Ok(())
    })();

    let _ = cleanup_path(&catalog_temp_path);
    let _ = cleanup_path(&metadata_temp_path);

    result
}

fn request_full_snapshot_plan(
    client: &Client,
    update_api_url: &str,
) -> Result<CatalogDownloadPlan> {
    let selection = api::fetch_full_snapshot_update_selection(client, update_api_url)?;

    match planner::plan_catalog_download(None, selection)? {
        Some(plan @ CatalogDownloadPlan::Full { .. }) => Ok(plan),
        _ => bail!("update API did not return a full snapshot plan"),
    }
}

fn clear_temp_file(path: &Path) -> Result<()> {
    cleanup_path(path).context("failed to clear previous catalog download")
}
