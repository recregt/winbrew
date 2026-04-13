use core::str::FromStr;

use serde::{Deserialize, Serialize};

use super::error::ModelError;

/// The semantic deployment outcome of an installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentKind {
    /// The package is installed with lasting system state.
    Installed,
    /// The package behaves like a portable deployment and can be removed by directory cleanup.
    Portable,
}

impl DeploymentKind {
    /// Return the canonical lowercase string used in persistence and logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Portable => "portable",
        }
    }
}

impl FromStr for DeploymentKind {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "installed" => Ok(Self::Installed),
            "portable" => Ok(Self::Portable),
            other => Err(ModelError::invalid_enum_value("deployment.kind", other)),
        }
    }
}

impl core::fmt::Display for DeploymentKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<DeploymentKind> for String {
    fn from(value: DeploymentKind) -> Self {
        value.to_string()
    }
}
