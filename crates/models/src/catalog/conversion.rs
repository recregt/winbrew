//! Conversion logic between raw catalog records and validated catalog types.
//!
//! This module owns the bridge from upstream schema-shaped payloads to the typed
//! catalog model used by the rest of the workspace. Keep parsing and
//! normalization rules here so the raw and typed layers stay clearly separated.

use core::convert::TryFrom;

use crate::catalog::package::{CatalogInstaller, CatalogPackage};
use crate::catalog::raw::{RawCatalogInstaller, RawCatalogPackage};
use crate::package::Package;
use crate::package::PackageId;
use crate::shared::ModelError;

impl From<&Package> for CatalogPackage {
    fn from(package: &Package) -> Self {
        let package_id = PackageId::parse(package.id.as_ref()).expect("package id should parse");

        Self {
            id: package.id.clone().into(),
            name: package.name.clone(),
            version: package.version.clone(),
            source: package_id.source(),
            namespace: package_id.namespace().map(str::to_string),
            source_id: package_id.source_id().to_string(),
            description: package.description.clone(),
            homepage: package.homepage.clone(),
            license: package.license.clone(),
            publisher: package.publisher.clone(),
        }
    }
}

impl TryFrom<RawCatalogPackage> for CatalogPackage {
    type Error = ModelError;

    fn try_from(raw: RawCatalogPackage) -> Result<Self, Self::Error> {
        let source = raw.source.parse()?;

        let package = Self {
            id: raw.id.into(),
            name: raw.name,
            version: raw.version.parse()?,
            source,
            namespace: raw.namespace,
            source_id: raw.source_id,
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
            nested_kind: raw.nested_kind.map(|kind| kind.parse()).transpose()?,
        };

        installer.validate()?;
        Ok(installer)
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogInstaller, CatalogPackage};
    use crate::catalog::raw::{RawCatalogInstaller, RawCatalogPackage};
    use crate::install::{Architecture, InstallerType};
    use crate::package::PackageSource;

    #[test]
    fn raw_catalog_package_converts_and_derives_source() {
        let package = RawCatalogPackage {
            id: "winget/Contoso.App".to_string(),
            name: "Contoso App".to_string(),
            version: "1.2.3".to_string(),
            source: "winget".to_string(),
            namespace: None,
            source_id: "Contoso.App".to_string(),
            description: Some("Example package".to_string()),
            homepage: None,
            license: None,
            publisher: Some("Contoso Ltd.".to_string()),
        };

        let converted = CatalogPackage::try_from(package).expect("raw package should convert");

        assert_eq!(converted.source, PackageSource::Winget);
        assert_eq!(converted.namespace, None);
        assert_eq!(converted.source_id, "Contoso.App");
        assert_eq!(converted.version.to_string(), "1.2.3");
    }

    #[test]
    fn raw_catalog_installer_converts() {
        let installer = RawCatalogInstaller {
            package_id: "winget/Contoso.App".to_string(),
            url: "https://example.test/app.exe".to_string(),
            hash: String::default(),
            arch: String::default(),
            kind: "portable".to_string(),
            nested_kind: Some("msi".to_string()),
        };

        let converted =
            CatalogInstaller::try_from(installer).expect("raw installer should convert");

        assert_eq!(converted.arch, Architecture::Any);
        assert_eq!(converted.kind, InstallerType::Portable);
        assert_eq!(converted.nested_kind, Some(InstallerType::Msi));
    }
}
