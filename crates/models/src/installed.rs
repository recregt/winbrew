use serde::{Deserialize, Serialize};

use crate::engine::{EngineKind, EngineMetadata};
use crate::installer::InstallerType;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PackageStatus {
    Installing,
    Ok,
    Updating,
    Failed,
}

impl PackageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Installing => "installing",
            Self::Ok => "ok",
            Self::Updating => "updating",
            Self::Failed => "failed",
        }
    }

    pub fn parse(status: &str) -> Self {
        match status {
            "ok" => Self::Ok,
            "updating" => Self::Updating,
            "failed" => Self::Failed,
            _ => Self::Installing,
        }
    }
}

impl std::fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub kind: InstallerType,
    pub engine_kind: EngineKind,
    pub engine_metadata: Option<EngineMetadata>,
    pub install_dir: String,
    pub msix_package_full_name: Option<String>,
    pub dependencies: Vec<String>,
    pub status: PackageStatus,
    pub installed_at: String,
}
