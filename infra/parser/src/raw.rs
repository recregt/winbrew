use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFetchedPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub publisher: Option<String>,
    #[serde(default)]
    pub installers: Vec<RawFetchedInstaller>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFetchedInstaller {
    pub url: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub arch: String,
    #[serde(rename = "type")]
    pub kind: String,
}
