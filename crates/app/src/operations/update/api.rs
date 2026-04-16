use anyhow::{Context, Result};

use crate::core::network::Client;
use crate::models::catalog::CatalogMetadata;

use super::types::CatalogUpdateResponse;

pub(super) fn fetch_catalog_update_selection(
    client: &Client,
    update_api_url: &str,
    local_metadata: Option<&CatalogMetadata>,
) -> Result<CatalogUpdateResponse> {
    fetch_catalog_update_selection_with_current_hash(
        client,
        update_api_url,
        local_metadata.map(|metadata| metadata.current_hash.as_str()),
    )
}

pub(super) fn fetch_full_snapshot_update_selection(
    client: &Client,
    update_api_url: &str,
) -> Result<CatalogUpdateResponse> {
    fetch_catalog_update_selection_with_current_hash(client, update_api_url, None)
}

fn fetch_catalog_update_selection_with_current_hash(
    client: &Client,
    update_api_url: &str,
    current_hash: Option<&str>,
) -> Result<CatalogUpdateResponse> {
    let api_url = current_hash
        .map(|hash| format!("{update_api_url}?current={hash}"))
        .unwrap_or_else(|| update_api_url.to_string());

    let response = client
        .get(api_url)
        .send()
        .context("failed to send catalog update selection request")?
        .error_for_status()
        .context("catalog update selection request failed")?;

    response
        .json()
        .context("failed to decode catalog update selection response")
}
