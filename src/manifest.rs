use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Binary,
    Msi,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BinEntry {
    Simple(String),
    Detailed {
        name: String,
        path: String,
        args: Option<String>,
    },
}

impl BinEntry {
    pub fn normalize(self) -> NormalizedBin {
        match self {
            BinEntry::Simple(path) => {
                let name = Path::new(&path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                NormalizedBin {
                    name,
                    path,
                    args: None,
                }
            }
            BinEntry::Detailed { name, path, args } => NormalizedBin { name, path, args },
        }
    }
}

#[derive(Debug)]
pub struct NormalizedBin {
    pub name: String,
    pub path: String,
    pub args: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Source {
    pub url: String,
    pub checksum: String,
    pub kind: PackageKind,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Install {
    #[serde(default)]
    pub bin: Vec<BinEntry>,
    #[serde(default)]
    pub strip_container: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Metadata {
    #[serde(default)]
    pub tags: Vec<String>,
    pub homepage: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub package: Package,
    pub source: Source,
    pub install: Install,
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

    pub fn normalized_bins(self) -> Vec<NormalizedBin> {
        self.install
            .bin
            .into_iter()
            .map(|b| b.normalize())
            .collect()
    }
}
