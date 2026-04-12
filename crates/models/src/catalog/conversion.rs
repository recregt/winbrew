use core::convert::TryFrom;

use crate::catalog::package::{CatalogInstaller, CatalogPackage};
use crate::catalog::raw::{RawCatalogInstaller, RawCatalogPackage};
use crate::package::Package;
use crate::package::PackageSource;
use crate::shared::ModelError;

impl From<&Package> for CatalogPackage {
    fn from(package: &Package) -> Self {
        Self {
            id: package.id.clone().into(),
            name: package.name.clone(),
            version: package.version.clone(),
            source: package.source,
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
            hash: String::default(),
            arch: String::default(),
            kind: "portable".to_string(),
        };

        let converted =
            CatalogInstaller::try_from(installer).expect("raw installer should convert");

        assert_eq!(converted.arch, Architecture::Any);
        assert_eq!(converted.kind, InstallerType::Portable);
    }
}
