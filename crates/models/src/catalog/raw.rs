use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCatalogPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCatalogInstaller {
    pub package_id: String,
    pub url: String,
    pub hash: String,
    pub arch: String,
    pub kind: String,
}
