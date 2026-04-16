use anyhow::Result;

use crate::core::network::Client;
use crate::models::catalog::CatalogMetadata;

use super::types::CatalogUpdateResponse;

pub(super) fn fetch_catalog_update_selection(
    client: &Client,
    update_api_url: &str,
    local_metadata: Option<&CatalogMetadata>,
) -> Result<Option<CatalogUpdateResponse>> {
    let api_url = local_metadata
        .map(|metadata| format!("{update_api_url}?current={}", metadata.current_hash))
        .unwrap_or_else(|| update_api_url.to_string());

    let request = client.get(api_url);

    let response = match request.send() {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };

    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };

    let selection: CatalogUpdateResponse = match response.json() {
        Ok(selection) => selection,
        Err(_) => return Ok(None),
    };

    Ok(Some(selection))
}
