//! Raw upstream catalog records before Winbrew validates and normalizes them.

use serde::{Deserialize, Serialize};

use crate::catalog::installer_type::CatalogInstallerType;
use crate::shared::HashAlgorithm;

/// Raw package payload exactly as it is received from the upstream feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCatalogPackage {
    /// Raw catalog id string.
    pub id: String,
    /// Raw package name.
    pub name: String,
    /// Raw version string.
    pub version: String,
    /// Upstream source string.
    pub source: String,
    /// Optional namespace or bucket within the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Source-local package identifier.
    pub source_id: String,
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
    /// Raw checksum algorithm.
    #[serde(default)]
    pub hash_algorithm: HashAlgorithm,
    /// Raw normalized installer family string.
    #[serde(default)]
    pub installer_type: CatalogInstallerType,
    /// Raw silent-install or package-manager switches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer_switches: Option<String>,
    /// Raw architecture string.
    pub arch: String,
    /// Raw installer kind string.
    pub kind: String,
    /// Raw nested installer kind string when the installer is archive-shaped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested_kind: Option<String>,
}
