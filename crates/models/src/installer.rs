use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::validation::{Validate, ensure_hash, ensure_http_url};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    X64,
    X86,
    Arm64,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallerType {
    Msi,
    Msix,
    Exe,
    Portable,
    Zip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installer {
    pub url: String,
    pub hash: String,
    pub architecture: Architecture,
    pub kind: InstallerType,
}

impl Installer {
    pub fn validate(&self) -> Result<(), ModelError> {
        ensure_http_url("installer.url", &self.url)?;
        ensure_hash("installer.hash", &self.hash)
    }
}

impl Validate for Installer {
    fn validate(&self) -> Result<(), ModelError> {
        Installer::validate(self)
    }
}

impl Architecture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::X86 => "x86",
            Self::Arm64 => "arm64",
            Self::Any => "",
        }
    }
}

impl InstallerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Msi => "msi",
            Self::Msix => "msix",
            Self::Exe => "exe",
            Self::Portable => "portable",
            Self::Zip => "zip",
        }
    }
}

impl core::fmt::Display for Architecture {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl core::fmt::Display for InstallerType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
