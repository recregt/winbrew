use core::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::install::installer::InstallerType;
use crate::package::PackageSource;
use crate::shared::ModelError;

/// Normalized installer family stored on catalog installer rows.
///
/// This is intentionally broader than the source-facing `InstallerType` enum.
/// It captures direct installer families as well as package-manager families
/// such as Scoop and Chocolatey.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogInstallerType {
    Msi,
    Msix,
    Appx,
    Msstore,
    Exe,
    Inno,
    Nullsoft,
    Wix,
    Burn,
    Pwa,
    Font,
    Portable,
    Zip,
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
            Self::Appx => "appx",
            Self::Msstore => "msstore",
            Self::Exe => "exe",
            Self::Inno => "inno",
            Self::Nullsoft => "nullsoft",
            Self::Wix => "wix",
            Self::Burn => "burn",
            Self::Pwa => "pwa",
            Self::Font => "font",
            Self::Portable => "portable",
            Self::Zip => "zip",
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
                InstallerType::Msix => Self::Msix,
                InstallerType::Appx => Self::Appx,
                InstallerType::Msi => Self::Msi,
                InstallerType::Exe => Self::Exe,
                InstallerType::Inno => Self::Inno,
                InstallerType::Nullsoft => Self::Nullsoft,
                InstallerType::Wix => Self::Wix,
                InstallerType::Burn => Self::Burn,
                InstallerType::Pwa => Self::Pwa,
                InstallerType::Font => Self::Font,
            },
            PackageSource::Winget | PackageSource::Winbrew => match kind {
                InstallerType::Portable if is_archive_url(url) => Self::Zip,
                InstallerType::Portable => Self::Portable,
                InstallerType::Zip => Self::Zip,
                InstallerType::Msix => Self::Msix,
                InstallerType::Appx => Self::Appx,
                InstallerType::Msi => Self::Msi,
                InstallerType::Exe => Self::Exe,
                InstallerType::Inno => Self::Inno,
                InstallerType::Nullsoft => Self::Nullsoft,
                InstallerType::Wix => Self::Wix,
                InstallerType::Burn => Self::Burn,
                InstallerType::Pwa => Self::Pwa,
                InstallerType::Font => Self::Font,
            },
        }
    }
}

impl FromStr for CatalogInstallerType {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "msi" => Ok(Self::Msi),
            "msix" => Ok(Self::Msix),
            "appx" => Ok(Self::Appx),
            "msstore" => Ok(Self::Msstore),
            "exe" => Ok(Self::Exe),
            "inno" => Ok(Self::Inno),
            "nsis" | "nullsoft" => Ok(Self::Nullsoft),
            "wix" => Ok(Self::Wix),
            "burn" => Ok(Self::Burn),
            "pwa" => Ok(Self::Pwa),
            "font" => Ok(Self::Font),
            "portable" => Ok(Self::Portable),
            "zip" => Ok(Self::Zip),
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

#[cfg(test)]
mod tests {
    use super::CatalogInstallerType;
    use crate::install::InstallerType;
    use crate::package::PackageSource;

    #[test]
    fn parses_nullsoft_alias() {
        assert_eq!(
            "nsis".parse::<CatalogInstallerType>().expect("nsis"),
            CatalogInstallerType::Nullsoft
        );
        assert_eq!(
            "nullsoft"
                .parse::<CatalogInstallerType>()
                .expect("nullsoft"),
            CatalogInstallerType::Nullsoft
        );
        assert_eq!(
            "msstore".parse::<CatalogInstallerType>().expect("msstore"),
            CatalogInstallerType::Msstore
        );
    }

    #[test]
    fn normalizes_raw_installer_families() {
        assert_eq!(
            CatalogInstallerType::normalize(
                PackageSource::Winget,
                InstallerType::Appx,
                "https://example.test/app.appx"
            ),
            CatalogInstallerType::Appx
        );
        assert_eq!(
            CatalogInstallerType::normalize(
                PackageSource::Winget,
                InstallerType::Portable,
                "https://example.test/app.exe"
            ),
            CatalogInstallerType::Portable
        );
        assert_eq!(
            CatalogInstallerType::normalize(
                PackageSource::Winget,
                InstallerType::Portable,
                "https://example.test/app.zip"
            ),
            CatalogInstallerType::Zip
        );
        assert_eq!(
            CatalogInstallerType::normalize(
                PackageSource::Winget,
                InstallerType::Nullsoft,
                "https://example.test/app.exe"
            ),
            CatalogInstallerType::Nullsoft
        );
    }
}
