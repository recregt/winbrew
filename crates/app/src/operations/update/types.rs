use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CatalogUpdateResponse {
    pub mode: CatalogUpdateMode,
    pub current: String,
    pub target: String,
    pub snapshot: Option<String>,
    #[serde(default)]
    pub patches: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum CatalogUpdateMode {
    Full,
    Patch,
}

#[derive(Debug, Clone)]
pub(super) enum CatalogDownloadPlan {
    Full {
        catalog_url: String,
        metadata_url: String,
        expected_hash: Option<String>,
    },
    Patch {
        patch_urls: Vec<String>,
        expected_hash: String,
    },
}
