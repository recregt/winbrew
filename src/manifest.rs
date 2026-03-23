use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ManifestInfo {
    #[serde(default = "default_manifest_type")]
    pub manifest_type: String,

    #[serde(default = "default_manifest_version")]
    pub manifest_version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub moniker: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Source {
    pub url: String,
    pub checksum: String,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InstallerSwitches {
    pub silent: Option<String>,

    pub silent_with_progress: Option<String>,

    pub interactive: Option<String>,

    pub custom: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InstallerEntry {
    pub architecture: String,

    pub installer_type: String,

    pub installer_url: String,

    pub installer_sha256: String,

    #[serde(default)]
    pub installer_locale: Option<String>,

    #[serde(default)]
    pub scope: Option<String>,

    #[serde(default)]
    pub product_code: Option<String>,

    #[serde(default)]
    pub release_date: Option<String>,

    #[serde(default)]
    pub display_name: Option<String>,

    #[serde(default)]
    pub upgrade_behavior: Option<String>,
}

impl InstallerEntry {
    pub fn to_source(&self) -> Source {
        Source {
            url: self.installer_url.clone(),
            checksum: self.installer_sha256.clone(),
            kind: self.installer_type.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Metadata {
    #[serde(default)]
    pub tags: Vec<String>,
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    #[serde(default)]
    pub manifest: ManifestInfo,

    pub package: Package,

    pub source: Source,

    #[serde(default)]
    pub installers: Vec<InstallerEntry>,

    pub metadata: Option<Metadata>,
}

impl Manifest {
    pub fn parse(content: &str) -> Result<Self> {
        Self::parse_toml(content)
    }

    pub fn parse_toml(content: &str) -> Result<Self> {
        toml::from_str(content).context("failed to parse manifest")
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).context("failed to read manifest file")?;
        Self::parse(&content)
    }

    pub fn preferred_installer(&self) -> Option<&InstallerEntry> {
        let preferred_arch = current_architecture();

        self.installers
            .iter()
            .find(|installer| installer.architecture.eq_ignore_ascii_case(preferred_arch))
            .or_else(|| self.installers.first())
    }

    pub fn selected_source(&self) -> Option<Source> {
        self.preferred_installer().map(|installer| Source {
            url: installer.installer_url.clone(),
            checksum: installer.installer_sha256.clone(),
            kind: installer.installer_type.clone(),
        })
    }

    pub fn validate_download_kind(&self) -> Result<()> {
        self.selected_source()
            .unwrap_or_else(|| self.source.clone())
            .validate_download_kind()
    }
}

fn current_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    }
}

impl Source {
    pub fn validate_download_kind(&self) -> Result<()> {
        let kind = self.kind.trim().to_ascii_lowercase();

        match kind.as_str() {
            "portable" | "msi" => Ok(()),
            other => anyhow::bail!("unsupported download type: {other}"),
        }
    }
}

fn default_manifest_type() -> String {
    "installer".to_string()
}

fn default_manifest_version() -> String {
    "1.9.0".to_string()
}
