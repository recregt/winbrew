use core::str::FromStr;
use serde::{Deserialize, Serialize};

use super::engine::EngineKind;
use crate::shared::DeploymentKind;
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
    /// Windows AppX package.
    Appx,
    /// Native executable installer.
    Exe,
    /// Inno Setup installer.
    Inno,
    /// Nullsoft installer.
    Nullsoft,
    /// WiX installer.
    Wix,
    /// Burn bootstrapper.
    Burn,
    /// Progressive Web App installer.
    Pwa,
    /// Font installer.
    Font,
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
            Self::Appx => "appx",
            Self::Exe => "exe",
            Self::Inno => "inno",
            Self::Nullsoft => "nullsoft",
            Self::Wix => "wix",
            Self::Burn => "burn",
            Self::Pwa => "pwa",
            Self::Font => "font",
            Self::Portable => "portable",
            Self::Zip => "zip",
        }
    }

    /// Return the semantic deployment outcome associated with this installer type.
    pub fn deployment_kind(self) -> DeploymentKind {
        self.into()
    }

    /// Return `true` when this installer comes from a Windows package family.
    pub fn is_windows_package(self) -> bool {
        matches!(self, Self::Msix | Self::Appx)
    }

    /// Return `true` when this installer belongs to an MSI-based family.
    pub fn is_msi_family(self) -> bool {
        matches!(self, Self::Msi | Self::Wix)
    }

    /// Return `true` when this installer belongs to a native executable family.
    pub fn is_native_exe_family(self) -> bool {
        matches!(self, Self::Exe | Self::Inno | Self::Nullsoft | Self::Burn)
    }

    /// Return `true` when this installer belongs to the Windows font family.
    pub fn is_font_family(self) -> bool {
        matches!(self, Self::Font)
    }

    /// Return `true` when this installer needs a dedicated special-case adapter.
    pub fn is_special_case(self) -> bool {
        matches!(self, Self::Pwa)
    }

    /// Return `true` when the payload is archive-shaped and should be unpacked.
    pub fn is_archive(self) -> bool {
        matches!(self, Self::Zip)
    }
}

impl FromStr for InstallerType {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "msi" => Ok(Self::Msi),
            "msix" => Ok(Self::Msix),
            "appx" => Ok(Self::Appx),
            "exe" => Ok(Self::Exe),
            "inno" => Ok(Self::Inno),
            "nullsoft" => Ok(Self::Nullsoft),
            "wix" => Ok(Self::Wix),
            "burn" => Ok(Self::Burn),
            "pwa" => Ok(Self::Pwa),
            "font" => Ok(Self::Font),
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
            EngineKind::Font => Self::Font,
        }
    }
}

impl From<InstallerType> for DeploymentKind {
    fn from(value: InstallerType) -> Self {
        match value {
            InstallerType::Portable | InstallerType::Zip => Self::Portable,
            _ => Self::Installed,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::DeploymentKind;

    use super::InstallerType;
    use core::str::FromStr;

    #[test]
    fn installer_type_parses_official_winget_values() {
        assert_eq!(
            InstallerType::from_str("appx").expect("appx"),
            InstallerType::Appx
        );
        assert_eq!(
            InstallerType::from_str("inno").expect("inno"),
            InstallerType::Inno
        );
        assert_eq!(
            InstallerType::from_str("nullsoft").expect("nullsoft"),
            InstallerType::Nullsoft
        );
        assert_eq!(
            InstallerType::from_str("wix").expect("wix"),
            InstallerType::Wix
        );
        assert_eq!(
            InstallerType::from_str("burn").expect("burn"),
            InstallerType::Burn
        );
        assert_eq!(
            InstallerType::from_str("pwa").expect("pwa"),
            InstallerType::Pwa
        );
        assert_eq!(
            InstallerType::from_str("font").expect("font"),
            InstallerType::Font
        );
    }

    #[test]
    fn installer_type_classifies_deployment_kind() {
        assert_eq!(
            InstallerType::Portable.deployment_kind(),
            DeploymentKind::Portable
        );
        assert_eq!(
            InstallerType::Zip.deployment_kind(),
            DeploymentKind::Portable
        );
        assert_eq!(
            InstallerType::Msi.deployment_kind(),
            DeploymentKind::Installed
        );
        assert_eq!(
            InstallerType::Exe.deployment_kind(),
            DeploymentKind::Installed
        );
        assert_eq!(
            InstallerType::Inno.deployment_kind(),
            DeploymentKind::Installed
        );
        assert_eq!(
            InstallerType::Nullsoft.deployment_kind(),
            DeploymentKind::Installed
        );
        assert_eq!(
            InstallerType::Burn.deployment_kind(),
            DeploymentKind::Installed
        );
        assert!(InstallerType::Msix.is_windows_package());
        assert!(InstallerType::Wix.is_msi_family());
        assert!(InstallerType::Exe.is_native_exe_family());
        assert!(InstallerType::Inno.is_native_exe_family());
        assert!(InstallerType::Nullsoft.is_native_exe_family());
        assert!(InstallerType::Burn.is_native_exe_family());
        assert!(InstallerType::Font.is_font_family());
        assert!(!InstallerType::Font.is_native_exe_family());
        assert!(!InstallerType::Font.is_special_case());
        assert!(InstallerType::Pwa.is_special_case());
    }
}
