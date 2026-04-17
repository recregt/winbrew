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
            created_at: None,
            updated_at: None,
            description: package.description.clone(),
            homepage: package.homepage.clone(),
            license: package.license.clone(),
            publisher: package.publisher.clone(),
            locale: None,
            moniker: None,
            platform: None,
            commands: None,
            protocols: None,
            file_extensions: None,
            capabilities: None,
            tags: None,
            bin: None,
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
            created_at: None,
            updated_at: None,
            description: raw.description,
            homepage: raw.homepage,
            license: raw.license,
            publisher: raw.publisher,
            locale: raw.locale,
            moniker: raw.moniker,
            platform: raw.platform,
            commands: raw.commands,
            protocols: raw.protocols,
            file_extensions: raw.file_extensions,
            capabilities: raw.capabilities,
            tags: raw.tags,
            bin: raw.bin,
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
            hash_algorithm: raw.hash_algorithm,
            installer_type: raw.installer_type,
            installer_switches: raw.installer_switches,
            platform: raw.platform,
            commands: raw.commands,
            protocols: raw.protocols,
            file_extensions: raw.file_extensions,
            capabilities: raw.capabilities,
            arch: raw.arch.parse()?,
            kind: raw.kind.parse()?,
            nested_kind: raw.nested_kind.map(|kind| kind.parse()).transpose()?,
            scope: raw.scope,
        };

        installer.validate()?;
        Ok(installer)
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogInstaller, CatalogPackage};
    use crate::catalog::installer_type::CatalogInstallerType;
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
            locale: Some("en-US".to_string()),
            moniker: Some("contoso".to_string()),
            platform: Some("[\"Windows.Desktop\"]".to_string()),
            commands: Some("[\"contoso\"]".to_string()),
            protocols: Some("[\"contoso-protocol\"]".to_string()),
            file_extensions: Some("[\".app\"]".to_string()),
            capabilities: Some("[\"internetClient\"]".to_string()),
            tags: Some("[\"utility\"]".to_string()),
            bin: Some("[\"tool.exe\"]".to_string()),
        };

        let converted = CatalogPackage::try_from(package).expect("raw package should convert");

        assert_eq!(converted.source, PackageSource::Winget);
        assert_eq!(converted.namespace, None);
        assert_eq!(converted.source_id, "Contoso.App");
        assert_eq!(converted.version.to_string(), "1.2.3");
        assert_eq!(converted.locale.as_deref(), Some("en-US"));
        assert_eq!(converted.moniker.as_deref(), Some("contoso"));
        assert_eq!(converted.platform.as_deref(), Some("[\"Windows.Desktop\"]"));
        assert_eq!(converted.commands.as_deref(), Some("[\"contoso\"]"));
        assert_eq!(
            converted.protocols.as_deref(),
            Some("[\"contoso-protocol\"]")
        );
        assert_eq!(converted.file_extensions.as_deref(), Some("[\".app\"]"));
        assert_eq!(
            converted.capabilities.as_deref(),
            Some("[\"internetClient\"]")
        );
        assert_eq!(converted.tags.as_deref(), Some("[\"utility\"]"));
        assert_eq!(converted.bin.as_deref(), Some("[\"tool.exe\"]"));
    }

    #[test]
    fn raw_catalog_installer_converts() {
        let installer = RawCatalogInstaller {
            package_id: "winget/Contoso.App".to_string(),
            url: "https://example.test/app.exe".to_string(),
            hash: String::default(),
            hash_algorithm: crate::shared::HashAlgorithm::Sha256,
            installer_type: CatalogInstallerType::Zip,
            installer_switches: Some("/S".to_string()),
            platform: Some("[\"Windows.Desktop\"]".to_string()),
            commands: Some("[\"contoso\"]".to_string()),
            protocols: Some("[\"contoso-protocol\"]".to_string()),
            file_extensions: Some("[\".exe\"]".to_string()),
            capabilities: Some("[\"internetClient\"]".to_string()),
            arch: String::default(),
            kind: "portable".to_string(),
            nested_kind: Some("msi".to_string()),
            scope: Some("machine".to_string()),
        };

        let converted =
            CatalogInstaller::try_from(installer).expect("raw installer should convert");

        assert_eq!(converted.arch, Architecture::Any);
        assert_eq!(converted.kind, InstallerType::Portable);
        assert_eq!(converted.nested_kind, Some(InstallerType::Msi));
        assert_eq!(converted.scope.as_deref(), Some("machine"));
        assert_eq!(converted.platform.as_deref(), Some("[\"Windows.Desktop\"]"));
        assert_eq!(converted.commands.as_deref(), Some("[\"contoso\"]"));
        assert_eq!(
            converted.protocols.as_deref(),
            Some("[\"contoso-protocol\"]")
        );
        assert_eq!(converted.file_extensions.as_deref(), Some("[\".exe\"]"));
        assert_eq!(
            converted.capabilities.as_deref(),
            Some("[\"internetClient\"]")
        );
        assert_eq!(
            converted.hash_algorithm,
            crate::shared::HashAlgorithm::Sha256
        );
        assert_eq!(converted.installer_type, CatalogInstallerType::Zip);
        assert_eq!(converted.installer_switches.as_deref(), Some("/S"));
    }
}
