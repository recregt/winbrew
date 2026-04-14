use core::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::install::installer::InstallerType;
use crate::package::PackageSource;
use crate::shared::ModelError;

/// Normalized installer family stored on catalog installer rows.
///
/// This is intentionally broader than the source-facing `InstallerType` enum.
/// It captures package-manager families such as Scoop and Chocolatey in
/// addition to direct installer families such as MSI, ZIP, and Inno.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogInstallerType {
    Msi,
    Msix,
    Exe,
    Inno,
    Nsis,
    Zip,
    Wix,
    Burn,
    Nuget,
    Scoop,
    #[default]
    Unknown,
}

impl CatalogInstallerType {
    /// Return the canonical storage string for this installer family.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Msi => "msi",
            Self::Msix => "msix",
            Self::Exe => "exe",
            Self::Inno => "inno",
            Self::Nsis => "nsis",
            Self::Zip => "zip",
            Self::Wix => "wix",
            Self::Burn => "burn",
            Self::Nuget => "nuget",
            Self::Scoop => "scoop",
            Self::Unknown => "unknown",
        }
    }

    /// Return `true` when the value is the fallback bucket.
    pub fn is_unknown(value: &Self) -> bool {
        matches!(value, Self::Unknown)
    }

    /// Normalize the installer family from source metadata and raw installer type.
    pub fn normalize(source: PackageSource, kind: InstallerType, url: &str) -> Self {
        match source {
            PackageSource::Chocolatey => Self::Nuget,
            PackageSource::Scoop => match kind {
                InstallerType::Portable if is_archive_url(url) => Self::Zip,
                InstallerType::Portable => Self::Scoop,
                InstallerType::Zip => Self::Zip,
                InstallerType::Msix | InstallerType::Appx => Self::Msix,
                InstallerType::Msi => Self::Msi,
                InstallerType::Exe => Self::Exe,
                InstallerType::Inno => Self::Inno,
                InstallerType::Nullsoft => Self::Nsis,
                InstallerType::Wix => Self::Wix,
                InstallerType::Burn => Self::Burn,
                InstallerType::Pwa | InstallerType::Font => Self::Unknown,
            },
            PackageSource::Winget | PackageSource::Winbrew => match kind {
                InstallerType::Portable if is_archive_url(url) => Self::Zip,
                InstallerType::Portable => Self::Unknown,
                InstallerType::Zip => Self::Zip,
                InstallerType::Msix | InstallerType::Appx => Self::Msix,
                InstallerType::Msi => Self::Msi,
                InstallerType::Exe => Self::Exe,
                InstallerType::Inno => Self::Inno,
                InstallerType::Nullsoft => Self::Nsis,
                InstallerType::Wix => Self::Wix,
                InstallerType::Burn => Self::Burn,
                InstallerType::Pwa | InstallerType::Font => Self::Unknown,
            },
        }
    }
}

impl FromStr for CatalogInstallerType {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "msi" => Ok(Self::Msi),
            "msix" | "appx" => Ok(Self::Msix),
            "exe" => Ok(Self::Exe),
            "inno" => Ok(Self::Inno),
            "nsis" | "nullsoft" => Ok(Self::Nsis),
            "zip" => Ok(Self::Zip),
            "wix" => Ok(Self::Wix),
            "burn" => Ok(Self::Burn),
            "nuget" => Ok(Self::Nuget),
            "scoop" => Ok(Self::Scoop),
            "unknown" => Ok(Self::Unknown),
            other => Err(ModelError::invalid_enum_value(
                "catalog_installer.installer_type",
                other,
            )),
        }
    }
}

impl core::fmt::Display for CatalogInstallerType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<CatalogInstallerType> for String {
    fn from(value: CatalogInstallerType) -> Self {
        value.to_string()
    }
}

impl AsRef<str> for CatalogInstallerType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

fn is_archive_url(url: &str) -> bool {
    let normalized = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim()
        .to_ascii_lowercase();

    normalized.ends_with(".zip")
        || normalized.ends_with(".7z")
        || normalized.ends_with(".rar")
        || normalized.ends_with(".tar")
        || normalized.ends_with(".tar.gz")
        || normalized.ends_with(".tgz")
        || normalized.ends_with(".tbz2")
}
