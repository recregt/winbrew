use core::str::FromStr;
use serde::{Deserialize, Serialize};

use super::engine::{EngineKind, EngineMetadata};
use super::installer::InstallerType;
use crate::shared::ModelError;

/// The persisted status of an installed package row.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PackageStatus {
    /// The package is currently being installed.
    Installing,
    /// The package is installed and healthy.
    Ok,
    /// The package is installed but an update is available.
    Updating,
    /// The package is known to be broken or failed.
    Failed,
}

impl PackageStatus {
    /// Return the canonical lowercase string used in persistence.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Installing => "installing",
            Self::Ok => "ok",
            Self::Updating => "updating",
            Self::Failed => "failed",
        }
    }
}

impl FromStr for PackageStatus {
    type Err = ModelError;

    fn from_str(status: &str) -> Result<Self, Self::Err> {
        match status.trim().to_ascii_lowercase().as_str() {
            "installing" => Ok(Self::Installing),
            "ok" => Ok(Self::Ok),
            "updating" => Ok(Self::Updating),
            "failed" => Ok(Self::Failed),
            other => Err(ModelError::invalid_enum_value("package.status", other)),
        }
    }
}

impl std::fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::PackageStatus;
    use core::str::FromStr;

    #[test]
    fn package_status_rejects_unknown_value() {
        let err = PackageStatus::from_str("mystery").expect_err("unknown status should fail");

        assert!(err.to_string().contains("invalid package.status: mystery"));
    }
}

/// The installed-package row persisted in Winbrew storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Installer format that produced the installation.
    pub kind: InstallerType,
    /// Engine kind that performed the install.
    pub engine_kind: EngineKind,
    /// Engine-specific metadata for repair and removal flows.
    pub engine_metadata: Option<EngineMetadata>,
    /// Final install directory.
    pub install_dir: String,
    /// Serialized dependency ids.
    pub dependencies: Vec<String>,
    /// Current package status.
    pub status: PackageStatus,
    /// Timestamp when the install was finalized.
    pub installed_at: String,
}
