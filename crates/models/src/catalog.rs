use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::package::PackageSource;
use crate::validation::{Validate, ensure_non_empty};
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
    pub arch: String,
    pub kind: String,
}

impl CatalogPackage {
    pub fn validate(&self) -> Result<(), ModelError> {
        ensure_non_empty("catalog_package.id", &self.id)?;
        ensure_non_empty("catalog_package.name", &self.name)?;
        self.version.validate()?;
        Ok(())
    }
}

impl Validate for CatalogPackage {
    fn validate(&self) -> Result<(), ModelError> {
        CatalogPackage::validate(self)
    }
}
