//! Engine metadata, install scope, and engine receipts.
//!
//! These types describe the execution layer that actually performs installs or
//! removals. The engine family keeps platform-specific details, install scope,
//! uninstall metadata, and MSI inventory snapshots together so storage and
//! repair code can persist the exact state reported by the engine.

use core::str::FromStr;
use serde::{Deserialize, Serialize};

use super::installer::InstallerType;
use crate::msi_inventory::MsiInventorySnapshot;
use crate::shared::ModelError;

/// The engine family that executed or will execute an install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    /// A Windows App Installer / MSIX flow.
    Msix,
    /// A zip extraction flow.
    Zip,
    /// A portable raw-copy flow.
    Portable,
    /// A native Windows MSI flow.
    Msi,
    /// A non-MSI executable flow.
    NativeExe,
    /// A per-user Windows font flow.
    Font,
}

/// The install scope reported by Windows package flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallScope {
    /// The package is installed for the current user.
    Installed,
    /// The package is provisioned at the machine level.
    Provisioned,
}

/// Engine-specific metadata attached to an installation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EngineMetadata {
    /// Metadata for an MSIX installation.
    Msix {
        /// Full package name reported by the Windows package system.
        package_full_name: String,
        /// Install scope reported by the engine.
        scope: InstallScope,
    },
    /// Metadata for an MSI installation.
    Msi {
        /// MSI product code used by uninstall and repair flows.
        product_code: String,
        /// Optional MSI upgrade code.
        upgrade_code: Option<String>,
        /// Install scope reported by the engine.
        scope: InstallScope,
        /// Registry keys touched by the installer.
        registry_keys: Vec<String>,
        /// Shortcuts touched by the installer.
        shortcuts: Vec<String>,
    },
    /// Metadata for a native executable installation.
    NativeExe {
        /// Quiet uninstall command published by the installer, if available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quiet_uninstall_command: Option<String>,
        /// Standard uninstall command published by the installer, if available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uninstall_command: Option<String>,
    },
}

/// Completion record returned by an engine after installation.
///
/// The receipt preserves the technical engine kind that executed the install,
/// the final install directory reported by the engine, and any engine-specific
/// metadata needed for future removal or repair. MSI engines may also attach
/// a complete inventory snapshot for database persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInstallReceipt {
    /// Engine that produced the receipt.
    pub engine_kind: EngineKind,
    /// Final install directory reported by the engine.
    pub install_dir: String,
    /// Optional MSI inventory snapshot collected during install.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msi_inventory_snapshot: Option<MsiInventorySnapshot>,
    /// Optional engine-specific metadata.
    pub engine_metadata: Option<EngineMetadata>,
}

impl EngineInstallReceipt {
    /// Build a receipt for the given engine kind, install directory, and metadata.
    pub fn new(
        engine_kind: EngineKind,
        install_dir: impl Into<String>,
        engine_metadata: Option<EngineMetadata>,
    ) -> Self {
        Self {
            engine_kind,
            install_dir: install_dir.into(),
            msi_inventory_snapshot: None,
            engine_metadata,
        }
    }

    /// Return the final install directory recorded in the receipt.
    pub fn install_dir(&self) -> &str {
        &self.install_dir
    }
}

impl EngineKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Msix => "msix",
            Self::Zip => "zip",
            Self::Portable => "portable",
            Self::Msi => "msi",
            Self::NativeExe => "nativeexe",
            Self::Font => "font",
        }
    }

    pub fn from_installer_type(kind: InstallerType) -> Self {
        match kind {
            InstallerType::Msix | InstallerType::Appx => Self::Msix,
            InstallerType::Msi | InstallerType::Wix => Self::Msi,
            InstallerType::Zip => Self::Zip,
            InstallerType::Portable => Self::Portable,
            InstallerType::Exe
            | InstallerType::Inno
            | InstallerType::Nullsoft
            | InstallerType::Burn
            | InstallerType::Pwa => Self::NativeExe,
            InstallerType::Font => Self::Font,
        }
    }
}

impl InstallScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Provisioned => "provisioned",
        }
    }
}

impl EngineMetadata {
    pub fn msix(package_full_name: impl Into<String>, scope: InstallScope) -> Self {
        Self::Msix {
            package_full_name: package_full_name.into(),
            scope,
        }
    }

    pub fn msix_package_full_name(&self) -> Option<&str> {
        match self {
            Self::Msix {
                package_full_name, ..
            } => Some(package_full_name.as_str()),
            Self::Msi { .. } | Self::NativeExe { .. } => None,
        }
    }

    /// Build native-executable metadata from discovered uninstall commands.
    pub fn native_exe(
        quiet_uninstall_command: Option<String>,
        uninstall_command: Option<String>,
    ) -> Self {
        Self::NativeExe {
            quiet_uninstall_command,
            uninstall_command,
        }
    }

    /// Return the quiet uninstall command if the metadata contains one.
    pub fn native_exe_quiet_uninstall_command(&self) -> Option<&str> {
        match self {
            Self::NativeExe {
                quiet_uninstall_command: Some(command),
                ..
            } => Some(command.as_str()),
            _ => None,
        }
    }

    /// Return the best available uninstall command for a native executable.
    pub fn native_exe_uninstall_command(&self) -> Option<&str> {
        match self {
            Self::NativeExe {
                quiet_uninstall_command: Some(command),
                ..
            } => Some(command.as_str()),
            Self::NativeExe {
                uninstall_command: Some(command),
                ..
            } => Some(command.as_str()),
            _ => None,
        }
    }
}

impl From<InstallerType> for EngineKind {
    fn from(value: InstallerType) -> Self {
        Self::from_installer_type(value)
    }
}

impl FromStr for EngineKind {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "msix" => Ok(Self::Msix),
            "zip" => Ok(Self::Zip),
            "portable" => Ok(Self::Portable),
            "msi" => Ok(Self::Msi),
            "nativeexe" => Ok(Self::NativeExe),
            "font" => Ok(Self::Font),
            other => Err(ModelError::invalid_enum_value("engine.kind", other)),
        }
    }
}

impl FromStr for InstallScope {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "installed" => Ok(Self::Installed),
            "provisioned" => Ok(Self::Provisioned),
            other => Err(ModelError::invalid_enum_value("engine.scope", other)),
        }
    }
}

impl core::fmt::Display for EngineKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<EngineKind> for String {
    fn from(value: EngineKind) -> Self {
        value.to_string()
    }
}

impl core::fmt::Display for InstallScope {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<InstallScope> for String {
    fn from(value: InstallScope) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineKind, EngineMetadata};
    use crate::install::installer::InstallerType;
    use core::str::FromStr;

    #[test]
    fn engine_kind_rejects_exe_alias() {
        let err = EngineKind::from_str("exe").expect_err("exe should not parse as an engine kind");

        assert!(err.to_string().contains("invalid engine.kind: exe"));
    }

    #[test]
    fn engine_kind_parses_font() {
        assert_eq!(
            EngineKind::from_str("font").expect("font"),
            EngineKind::Font
        );
        assert_eq!(EngineKind::Font.to_string(), "font");
    }

    #[test]
    fn engine_kind_round_trips_font_to_installer_type() {
        assert_eq!(InstallerType::from(EngineKind::Font), InstallerType::Font);
    }

    #[test]
    fn native_exe_metadata_prefers_quiet_uninstall_command() {
        let metadata = EngineMetadata::native_exe(
            Some("C:\\Apps\\Demo\\uninstall.exe /S".to_string()),
            Some("C:\\Apps\\Demo\\uninstall.exe".to_string()),
        );

        assert_eq!(
            metadata.native_exe_quiet_uninstall_command(),
            Some("C:\\Apps\\Demo\\uninstall.exe /S")
        );
        assert_eq!(
            metadata.native_exe_uninstall_command(),
            Some("C:\\Apps\\Demo\\uninstall.exe /S")
        );
    }

    #[test]
    fn native_exe_metadata_falls_back_to_uninstall_command() {
        let metadata =
            EngineMetadata::native_exe(None, Some("C:\\Apps\\Demo\\uninstall.exe".to_string()));

        assert_eq!(metadata.native_exe_quiet_uninstall_command(), None);
        assert_eq!(
            metadata.native_exe_uninstall_command(),
            Some("C:\\Apps\\Demo\\uninstall.exe")
        );
        assert_eq!(metadata.msix_package_full_name(), None);
    }
}
