use anyhow::Result;

use crate::models::catalog::CatalogMetadata;

use super::metadata::metadata_url_for_snapshot_url;
use super::types::{CatalogDownloadPlan, CatalogUpdateMode, CatalogUpdateResponse};

/// Converts the update API response into a concrete catalog download plan.
///
/// The planner is intentionally small and deterministic: it validates the API
/// payload against the caller's local metadata, then maps the response into one
/// of three outcomes.
///
/// - `Current`: the catalog is already up to date and no download is needed.
/// - `Full`: the workflow should download a full snapshot plus its metadata.
/// - `Patch`: the workflow should apply one or more incremental SQL patches to
///   the existing local catalog.
///
/// The function returns `Ok(None)` when the response is incomplete or
/// incompatible with the local catalog state, which lets the caller fall back
/// to a full snapshot request.
pub(super) fn plan_catalog_download(
    local_metadata: Option<&CatalogMetadata>,
    selection: CatalogUpdateResponse,
) -> Result<Option<CatalogDownloadPlan>> {
    match selection.mode {
        CatalogUpdateMode::Current => {
            let current_hash = if selection.current.trim().is_empty() {
                selection.target.clone()
            } else {
                selection.current.clone()
            };

            if current_hash.trim().is_empty() {
                return Ok(None);
            }

            let target_hash = if selection.target.trim().is_empty() {
                current_hash.clone()
            } else {
                selection.target
            };

            Ok(Some(CatalogDownloadPlan::Current {
                current_hash,
                target_hash,
            }))
        }
        CatalogUpdateMode::Full => {
            if selection.target.trim().is_empty() {
                return Ok(None);
            }

            let catalog_url = match selection.snapshot {
                Some(snapshot) if !snapshot.trim().is_empty() => snapshot,
                _ => return Ok(None),
            };

            let metadata_url = metadata_url_for_snapshot_url(&catalog_url)?;

            Ok(Some(CatalogDownloadPlan::Full {
                catalog_url,
                metadata_url,
                expected_hash: Some(selection.target),
            }))
        }
        CatalogUpdateMode::Patch => {
            if selection.target.trim().is_empty() {
                return Ok(None);
            }

            let local_metadata = match local_metadata {
                Some(metadata) => metadata,
                None => return Ok(None),
            };

            if local_metadata.current_hash != selection.current {
                return Ok(None);
            }

            if selection.patches.is_empty()
                || selection.patches.iter().any(|url| url.trim().is_empty())
            {
                return Ok(None);
            }

            Ok(Some(CatalogDownloadPlan::Patch {
                patch_urls: selection.patches,
                expected_hash: selection.target,
            }))
        }
    }
}
