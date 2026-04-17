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
    /// Optional package metadata locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Optional package moniker or alias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moniker: Option<String>,
    /// Optional package platform metadata encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Optional package commands encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<String>,
    /// Optional package protocols encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<String>,
    /// Optional package file extensions encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_extensions: Option<String>,
    /// Optional package capabilities encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<String>,
    /// Optional package search tags encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Optional package bin metadata encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
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
    pub hash_algorithm: HashAlgorithm,
    /// Raw normalized installer family string.
    pub installer_type: CatalogInstallerType,
    /// Raw silent-install or package-manager switches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer_switches: Option<String>,
    /// Raw platform metadata encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Raw commands encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<String>,
    /// Raw protocols encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<String>,
    /// Raw file extensions encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_extensions: Option<String>,
    /// Raw capabilities encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<String>,
    /// Raw architecture string.
    pub arch: String,
    /// Raw installer kind string.
    pub kind: String,
    /// Raw nested installer kind string when the installer is archive-shaped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested_kind: Option<String>,
    /// Raw install scope string when the source provides one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}
