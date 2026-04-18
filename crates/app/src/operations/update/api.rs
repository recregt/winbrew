use anyhow::{Context, Result};
use url::Url;

use crate::core::network::Client;
use crate::models::catalog::CatalogMetadata;

use super::types::CatalogUpdateResponse;

/// Fetches the catalog update selection.
///
/// When local metadata is available, the current catalog hash is sent to the
/// update API so it can choose between a current, patch, or full-snapshot
/// response.
///
/// # Errors
/// Returns an error when the request URL is invalid, the request fails, the
/// API returns a non-success status, or the response body cannot be decoded.
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

/// Fetches a full snapshot update selection.
///
/// This bypasses local metadata and always asks the API for a full snapshot
/// plan.
///
/// # Errors
/// Returns an error when the request URL is invalid, the request fails, the
/// API returns a non-success status, or the response body cannot be decoded.
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
    let api_url = build_update_api_url(update_api_url, current_hash).with_context(|| {
        format!("failed to build catalog update selection URL from {update_api_url}")
    })?;

    let response = client
        .get(api_url.as_str())
        .send()
        .with_context(|| format!("failed to send catalog update selection request to {api_url}"))?
        .error_for_status()
        .with_context(|| format!("catalog update selection request failed for {api_url}"))?;

    response.json().with_context(|| {
        format!("failed to decode catalog update selection response from {api_url}")
    })
}

fn build_update_api_url(update_api_url: &str, current_hash: Option<&str>) -> Result<Url> {
    let mut url = Url::parse(update_api_url).context("invalid catalog update selection API URL")?;

    if let Some(current_hash) = current_hash {
        url.query_pairs_mut().append_pair("current", current_hash);
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::build_update_api_url;

    #[test]
    fn build_update_api_url_appends_current_hash() {
        let url = build_update_api_url("https://example.invalid/v1/update", Some("sha256:abc123"))
            .expect("build url");

        assert_eq!(
            url.as_str(),
            "https://example.invalid/v1/update?current=sha256%3Aabc123"
        );
    }

    #[test]
    fn build_update_api_url_preserves_existing_query_parameters() {
        let url = build_update_api_url(
            "https://example.invalid/v1/update?feature=preview",
            Some("sha256:abc123"),
        )
        .expect("build url");

        assert_eq!(
            url.as_str(),
            "https://example.invalid/v1/update?feature=preview&current=sha256%3Aabc123"
        );
    }
}
