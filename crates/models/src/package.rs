use serde::{Deserialize, Serialize};

use crate::dependency::Dependency;
use crate::error::ModelError;
use crate::installer::Installer;
use crate::validation::{Validate, ensure_non_empty};
use crate::version::Version;

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

impl core::fmt::Display for PackageSource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
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

impl core::fmt::Display for PackageKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{Package, PackageKind, PackageSource};
    use crate::installer::{Architecture, Installer, InstallerType};
    use crate::version::Version;

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
}
