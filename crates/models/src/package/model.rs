use core::str::FromStr;
use serde::{Deserialize, Serialize};

use crate::Installer;
use crate::shared::validation::{Validate, ensure_non_empty};
use crate::shared::{ModelError, Version};

use super::dependency::Dependency;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageSource {
    Winget,
    Scoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Catalog,
    Installed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: Version,
    pub source: PackageSource,
    pub kind: PackageKind,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub publisher: Option<String>,
    pub installers: Vec<Installer>,
    pub dependencies: Vec<Dependency>,
}

impl Package {
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
        }
    }

    pub fn from_catalog_id(id: &str) -> Self {
        match id.split_once('/') {
            Some(("scoop", _)) => Self::Scoop,
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
    use crate::shared::Version;
    use crate::{Architecture, Installer, InstallerType};
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
            PackageKind::from_str("catalog").unwrap(),
            PackageKind::Catalog
        );
        assert_eq!(
            PackageKind::from_str("installed").unwrap(),
            PackageKind::Installed
        );
    }
}
