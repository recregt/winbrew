use core::str::FromStr;
use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::installer::InstallerType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    Msix,
    Zip,
    Portable,
    Msi,
    NativeExe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallScope {
    Installed,
    Provisioned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EngineMetadata {
    Msix {
        package_full_name: String,
        scope: InstallScope,
    },
    Msi {
        product_code: String,
        upgrade_code: Option<String>,
        scope: InstallScope,
        registry_keys: Vec<String>,
        shortcuts: Vec<String>,
    },
}

/// Completion record returned by an engine after installation.
///
/// The receipt preserves the technical engine kind that executed the install,
/// the final install directory reported by the engine, and any engine-specific
/// metadata needed for future removal or repair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInstallReceipt {
    pub engine_kind: EngineKind,
    pub install_dir: String,
    pub engine_metadata: Option<EngineMetadata>,
}

impl EngineInstallReceipt {
    pub fn new(
        engine_kind: EngineKind,
        install_dir: impl Into<String>,
        engine_metadata: Option<EngineMetadata>,
    ) -> Self {
        Self {
            engine_kind,
            install_dir: install_dir.into(),
            engine_metadata,
        }
    }

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
        }
    }

    pub fn from_installer_type(kind: InstallerType) -> Self {
        match kind {
            InstallerType::Msix => Self::Msix,
            InstallerType::Zip => Self::Zip,
            InstallerType::Portable => Self::Portable,
            InstallerType::Msi => Self::Msi,
            InstallerType::Exe => Self::NativeExe,
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
            Self::Msi { .. } => None,
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
    use super::EngineKind;
    use core::str::FromStr;

    #[test]
    fn engine_kind_rejects_exe_alias() {
        let err = EngineKind::from_str("exe").expect_err("exe should not parse as an engine kind");

        assert!(err.to_string().contains("invalid engine.kind: exe"));
    }
}
