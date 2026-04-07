use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::installer::{Architecture, InstallerType};
use crate::package::PackageSource;
use crate::validation::{Validate, ensure_hash, ensure_http_url, ensure_non_empty};
use crate::version::Version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPackage {
    pub id: String,
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
    pub package_id: String,
    pub url: String,
    pub hash: String,
    pub arch: Architecture,
    pub kind: InstallerType,
}

impl CatalogPackage {
    pub fn validate(&self) -> Result<(), ModelError> {
        ensure_non_empty("catalog_package.id", &self.id)?;
        ensure_non_empty("catalog_package.name", &self.name)?;
        self.version.validate()?;

        let expected_source = PackageSource::from_catalog_id(&self.id);
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
        ensure_non_empty("catalog_installer.package_id", &self.package_id)?;
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
