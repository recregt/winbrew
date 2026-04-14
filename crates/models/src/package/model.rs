//! Package aggregate types and source/kind classification.
//!
//! This file owns the canonical typed package representation used by catalog,
//! search, and install orchestration. The aggregate keeps the source metadata,
//! version, optional descriptive fields, installer candidates, and dependency
//! list together so callers do not need to reconstruct a package from multiple
//! sources.

use core::str::FromStr;
use serde::{Deserialize, Serialize};

use crate::install::Installer;
use crate::shared::validation::{Validate, ensure_non_empty};
use crate::shared::{ModelError, Version};

use super::dependency::Dependency;

/// The upstream source that produced a package record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageSource {
    /// A package sourced from the Winget catalog.
    Winget,
    /// A package sourced from a Scoop bucket.
    Scoop,
    /// A package sourced from Chocolatey.
    Chocolatey,
    /// A package sourced from the WinBrew catalog.
    Winbrew,
}

/// The lifecycle classification of a package record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    /// A catalog record that can be installed.
    Catalog,
    /// A record that represents an installed package snapshot.
    Installed,
}

/// Canonical package aggregate used by catalog, search, and install flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    /// Stable package identifier in canonical catalog id form.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Parsed semantic version.
    pub version: Version,
    /// The source that produced the record.
    pub source: PackageSource,
    /// Whether the record represents a catalog item or installed snapshot.
    pub kind: PackageKind,
    /// Short summary or description text, when available.
    pub description: Option<String>,
    /// Product homepage URL, when provided.
    pub homepage: Option<String>,
    /// License string reported by the source.
    pub license: Option<String>,
    /// Publisher or maintainer string.
    pub publisher: Option<String>,
    /// Resolved installer candidates associated with the package.
    pub installers: Vec<Installer>,
    /// Declared dependencies for the package.
    pub dependencies: Vec<Dependency>,
}

impl Package {
    /// Validate the package and all nested installer/dependency records.
    pub fn validate(&self) -> Result<(), ModelError> {
        ensure_non_empty("package.id", &self.id)?;
        ensure_non_empty("package.name", &self.name)?;
        self.version.validate()?;

        for installer in &self.installers {
            installer.validate()?;
        }

        for dependency in &self.dependencies {
            dependency.validate()?;
        }

        Ok(())
    }
}

impl PackageSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Winget => "winget",
            Self::Scoop => "scoop",
            Self::Chocolatey => "chocolatey",
            Self::Winbrew => "winbrew",
        }
    }

    pub fn from_catalog_id(id: &str) -> Self {
        match id.split_once('/') {
            Some(("scoop", _)) => Self::Scoop,
            Some(("chocolatey", _)) => Self::Chocolatey,
            Some(("winbrew", _)) => Self::Winbrew,
            _ => Self::Winget,
        }
    }
}

impl FromStr for PackageSource {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "winget" => Ok(Self::Winget),
            "scoop" => Ok(Self::Scoop),
            "chocolatey" => Ok(Self::Chocolatey),
            "winbrew" => Ok(Self::Winbrew),
            other => Err(ModelError::invalid_enum_value("package.source", other)),
        }
    }
}

impl core::fmt::Display for PackageSource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<PackageSource> for String {
    fn from(value: PackageSource) -> Self {
        value.to_string()
    }
}

impl AsRef<str> for PackageSource {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Installed => "installed",
        }
    }
}

impl FromStr for PackageKind {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "catalog" => Ok(Self::Catalog),
            "installed" => Ok(Self::Installed),
            other => Err(ModelError::invalid_enum_value("package.kind", other)),
        }
    }
}

impl core::fmt::Display for PackageKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<PackageKind> for String {
    fn from(value: PackageKind) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{Package, PackageKind, PackageSource};
    use crate::install::{Architecture, Installer, InstallerType};
    use crate::shared::Version;
    use core::str::FromStr;

    #[test]
    fn validates_package() {
        let package = Package {
            id: "winget/Contoso.App".to_string(),
            name: "Contoso App".to_string(),
            version: Version::parse("1.2.3").expect("version should parse"),
            source: PackageSource::Winget,
            kind: PackageKind::Catalog,
            description: None,
            homepage: None,
            license: None,
            publisher: None,
            installers: vec![Installer {
                url: "https://example.test/app.exe".to_string(),
                hash: "sha256:deadbeef".to_string(),
                architecture: Architecture::X64,
                kind: InstallerType::Exe,
            }],
            dependencies: vec![],
        };

        assert!(package.validate().is_ok());
    }

    #[test]
    fn parses_package_source_and_kind() {
        assert_eq!(
            PackageSource::from_str("winget").unwrap(),
            PackageSource::Winget
        );
        assert_eq!(
            PackageSource::from_str("scoop").unwrap(),
            PackageSource::Scoop
        );
        assert_eq!(
            PackageSource::from_str("chocolatey").unwrap(),
            PackageSource::Chocolatey
        );
        assert_eq!(
            PackageSource::from_str("winbrew").unwrap(),
            PackageSource::Winbrew
        );
        assert_eq!(
            PackageKind::from_str("catalog").unwrap(),
            PackageKind::Catalog
        );
        assert_eq!(
            PackageKind::from_str("installed").unwrap(),
            PackageKind::Installed
        );
    }
}
