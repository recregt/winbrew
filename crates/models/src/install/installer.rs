use core::str::FromStr;
use serde::{Deserialize, Serialize};

use super::engine::EngineKind;
use crate::shared::ModelError;
use crate::shared::validation::{Validate, ensure_hash, ensure_http_url};

/// The target architecture of an installer payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    /// x64 / amd64 payload.
    X64,
    /// x86 payload.
    X86,
    /// ARM64 payload.
    Arm64,
    /// Architecture-neutral payload.
    Any,
}

/// The installer format represented by a catalog record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallerType {
    /// Windows Installer package.
    Msi,
    /// Windows App Installer / MSIX package.
    Msix,
    /// Native executable installer.
    Exe,
    /// Portable archive or copy-based package.
    Portable,
    /// Zip archive installer.
    Zip,
}

/// A resolved installer candidate for a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installer {
    /// Download URL for the installer.
    pub url: String,
    /// Checksum used for verification.
    pub hash: String,
    /// Target architecture.
    pub architecture: Architecture,
    /// Installer format.
    pub kind: InstallerType,
}

impl Installer {
    /// Validate the URL and checksum contract for the installer.
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
    /// Return the canonical display string for the architecture.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::X86 => "x86",
            Self::Arm64 => "arm64",
            Self::Any => "",
        }
    }

    /// Return the current host architecture when it can be classified.
    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "x86_64" => Self::X64,
            "x86" => Self::X86,
            "aarch64" => Self::Arm64,
            _ => Self::Any,
        }
    }
}

impl FromStr for Architecture {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "x64" => Ok(Self::X64),
            "x86" => Ok(Self::X86),
            "arm64" => Ok(Self::Arm64),
            "" => Ok(Self::Any),
            other => Err(ModelError::invalid_enum_value("installer.arch", other)),
        }
    }
}

impl InstallerType {
    /// Return the canonical display string for the installer format.
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

impl FromStr for InstallerType {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "msi" => Ok(Self::Msi),
            "msix" => Ok(Self::Msix),
            "exe" => Ok(Self::Exe),
            "portable" => Ok(Self::Portable),
            "zip" => Ok(Self::Zip),
            other => Err(ModelError::invalid_enum_value("installer.kind", other)),
        }
    }
}

impl core::fmt::Display for Architecture {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<Architecture> for String {
    fn from(value: Architecture) -> Self {
        value.to_string()
    }
}

impl core::fmt::Display for InstallerType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<InstallerType> for String {
    fn from(value: InstallerType) -> Self {
        value.to_string()
    }
}

impl From<EngineKind> for InstallerType {
    fn from(value: EngineKind) -> Self {
        match value {
            EngineKind::Msix => Self::Msix,
            EngineKind::Zip => Self::Zip,
            EngineKind::Portable => Self::Portable,
            EngineKind::Msi => Self::Msi,
            EngineKind::NativeExe => Self::Exe,
        }
    }
}
