//! Raw upstream catalog records before Winbrew validates and normalizes them.

use serde::{Deserialize, Serialize};

/// Raw package payload exactly as it is received from the upstream feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCatalogPackage {
    /// Raw catalog id string.
    pub id: String,
    /// Raw package name.
    pub name: String,
    /// Raw version string.
    pub version: String,
    /// Optional upstream source string.
    pub source: Option<String>,
    /// Optional description text.
    pub description: Option<String>,
    /// Optional homepage URL.
    pub homepage: Option<String>,
    /// Optional license string.
    pub license: Option<String>,
    /// Optional publisher string.
    pub publisher: Option<String>,
}

/// Raw installer payload exactly as it is received from the upstream feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCatalogInstaller {
    /// Raw package id string.
    pub package_id: String,
    /// Raw installer URL.
    pub url: String,
    /// Raw checksum string.
    pub hash: String,
    /// Raw architecture string.
    pub arch: String,
    /// Raw installer kind string.
    pub kind: String,
}
