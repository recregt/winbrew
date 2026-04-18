use serde::Deserialize;

/// Data transfer objects for catalog update selection and download planning.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct CatalogUpdateResponse {
    /// The update strategy selected by the API.
    pub mode: CatalogUpdateMode,
    /// The catalog hash the API considers current for the local client.
    pub current: String,
    /// The hash the refreshed catalog should end up with.
    pub target: String,
    /// The full snapshot URL when the API returns a full snapshot plan.
    pub snapshot: Option<String>,
    /// Ordered patch URLs when the API returns a patch plan.
    #[serde(default)]
    pub patches: Vec<String>,
}

/// The mode returned by the catalog update API.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum CatalogUpdateMode {
    /// The catalog is already up to date.
    Current,
    /// Download the full snapshot and rebuild local state.
    Full,
    /// Apply incremental SQL patches to the existing catalog.
    Patch,
}

/// The internal plan used by the refresh workflow after API selection.
#[derive(Debug, Clone)]
pub(super) enum CatalogDownloadPlan {
    /// No refresh work is needed because the hashes already match.
    Current {
        /// The hash reported as current by the API.
        current_hash: String,
        /// The hash the API expects after refresh.
        target_hash: String,
    },
    /// Download a complete snapshot and its metadata.
    Full {
        /// URL for the compressed catalog snapshot.
        catalog_url: String,
        /// URL for the matching metadata JSON.
        metadata_url: String,
        /// Expected hash of the downloaded catalog snapshot.
        expected_hash: Option<String>,
    },
    /// Apply one or more ordered SQL patch files.
    Patch {
        /// Ordered patch URLs to apply.
        patch_urls: Vec<String>,
        /// Expected hash after all patches are applied.
        expected_hash: String,
    },
}
