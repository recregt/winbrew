use core::str::FromStr;
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
}

impl FromStr for PackageStatus {
    type Err = crate::error::ModelError;

    fn from_str(status: &str) -> Result<Self, Self::Err> {
        match status.trim().to_ascii_lowercase().as_str() {
            "installing" => Ok(Self::Installing),
            "ok" => Ok(Self::Ok),
            "updating" => Ok(Self::Updating),
            "failed" => Ok(Self::Failed),
            other => Err(crate::error::ModelError::invalid_enum_value(
                "package.status",
                other,
            )),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub kind: InstallerType,
    pub engine_kind: EngineKind,
    pub engine_metadata: Option<EngineMetadata>,
    pub install_dir: String,
    pub dependencies: Vec<String>,
    pub status: PackageStatus,
    pub installed_at: String,
}
