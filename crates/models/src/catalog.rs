use serde::{Deserialize, Serialize};

use core::convert::TryFrom;

use crate::error::ModelError;
use crate::identifiers::CatalogId;
use crate::installer::{Architecture, InstallerType};
use crate::package::PackageSource;
use crate::validation::{Validate, ensure_hash, ensure_http_url, ensure_non_empty};
use crate::version::Version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPackage {
    pub id: CatalogId,
    pub name: String,
    pub version: Version,
    pub source: PackageSource,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogInstaller {
    pub package_id: CatalogId,
    pub url: String,
    pub hash: String,
    pub arch: Architecture,
    pub kind: InstallerType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCatalogPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCatalogInstaller {
    pub package_id: String,
    pub url: String,
    pub hash: String,
    pub arch: String,
    pub kind: String,
}

impl CatalogPackage {
    pub fn validate(&self) -> Result<(), ModelError> {
        self.id.validate()?;
        ensure_non_empty("catalog_package.name", &self.name)?;
        self.version.validate()?;

        let expected_source = PackageSource::from_catalog_id(self.id.as_ref());
        if self.source != expected_source {
            return Err(ModelError::source_mismatch(
                "catalog_package.source",
                expected_source,
                self.source,
            ));
        }

        Ok(())
    }
}

impl Validate for CatalogPackage {
    fn validate(&self) -> Result<(), ModelError> {
        CatalogPackage::validate(self)
    }
}

impl CatalogInstaller {
    pub fn validate(&self) -> Result<(), ModelError> {
        self.package_id.validate()?;
        ensure_http_url("catalog_installer.url", &self.url)?;

        if !self.hash.trim().is_empty() {
            ensure_hash("catalog_installer.hash", &self.hash)?;
        }

        Ok(())
    }
}

impl Validate for CatalogInstaller {
    fn validate(&self) -> Result<(), ModelError> {
        CatalogInstaller::validate(self)
    }
}

impl TryFrom<RawCatalogPackage> for CatalogPackage {
    type Error = ModelError;

    fn try_from(raw: RawCatalogPackage) -> Result<Self, Self::Error> {
        let source = raw
            .source
            .as_deref()
            .map(str::parse)
            .transpose()?
            .unwrap_or_else(|| PackageSource::from_catalog_id(&raw.id));

        let package = Self {
            id: raw.id.into(),
            name: raw.name,
            version: raw.version.parse()?,
            source,
            description: raw.description,
            homepage: raw.homepage,
            license: raw.license,
            publisher: raw.publisher,
        };

        package.validate()?;
        Ok(package)
    }
}

impl TryFrom<RawCatalogInstaller> for CatalogInstaller {
    type Error = ModelError;

    fn try_from(raw: RawCatalogInstaller) -> Result<Self, Self::Error> {
        let installer = Self {
            package_id: raw.package_id.into(),
            url: raw.url,
            hash: raw.hash,
            arch: raw.arch.parse()?,
            kind: raw.kind.parse()?,
        };

        installer.validate()?;
        Ok(installer)
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogInstaller, CatalogPackage, RawCatalogInstaller, RawCatalogPackage};
    use crate::installer::{Architecture, InstallerType};
    use crate::package::PackageSource;
    use crate::version::Version;

    #[test]
    fn rejects_source_mismatch() {
        let package = CatalogPackage {
            id: "winget/Contoso.App".into(),
            name: "Contoso App".to_string(),
            version: Version::parse("1.2.3").expect("version should parse"),
            source: PackageSource::Scoop,
            description: None,
            homepage: None,
            license: None,
            publisher: None,
        };

        let err = package.validate().expect_err("source mismatch should fail");

        assert!(err.to_string().contains("source mismatch"));
    }

    #[test]
    fn validates_checksumless_catalog_installer() {
        let installer = CatalogInstaller {
            package_id: "winget/Contoso.App".into(),
            url: "https://example.test/app.exe".to_string(),
            hash: String::new(),
            arch: Architecture::Any,
            kind: InstallerType::Portable,
        };

        assert!(installer.validate().is_ok());
    }

    #[test]
    fn catalog_package_round_trips_through_serde() {
        let package = CatalogPackage {
            id: "scoop/main/Contoso.App".into(),
            name: "Contoso App".to_string(),
            version: Version::parse("1.2.3").expect("version should parse"),
            source: PackageSource::Scoop,
            description: Some("Example package".to_string()),
            homepage: None,
            license: None,
            publisher: Some("Contoso Ltd.".to_string()),
        };

        let json = serde_json::to_string(&package).expect("package should serialize");
        let restored: CatalogPackage =
            serde_json::from_str(&json).expect("package should deserialize");

        assert_eq!(restored.id, package.id);
        assert_eq!(restored.source, package.source);
        assert_eq!(restored.version, package.version);
        assert_eq!(restored.publisher, package.publisher);
    }

    #[test]
    fn raw_catalog_package_converts_and_derives_source() {
        let package = RawCatalogPackage {
            id: "winget/Contoso.App".to_string(),
            name: "Contoso App".to_string(),
            version: "1.2.3".to_string(),
            source: None,
            description: Some("Example package".to_string()),
            homepage: None,
            license: None,
            publisher: Some("Contoso Ltd.".to_string()),
        };

        let converted = CatalogPackage::try_from(package).expect("raw package should convert");

        assert_eq!(converted.source, PackageSource::Winget);
        assert_eq!(converted.version.to_string(), "1.2.3");
    }

    #[test]
    fn raw_catalog_installer_converts() {
        let installer = RawCatalogInstaller {
            package_id: "winget/Contoso.App".to_string(),
            url: "https://example.test/app.exe".to_string(),
            hash: String::new(),
            arch: String::new(),
            kind: "portable".to_string(),
        };

        let converted =
            CatalogInstaller::try_from(installer).expect("raw installer should convert");

        assert_eq!(converted.arch, Architecture::Any);
        assert_eq!(converted.kind, InstallerType::Portable);
    }
}
