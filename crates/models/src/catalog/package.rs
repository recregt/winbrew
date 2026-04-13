use serde::{Deserialize, Serialize};

use crate::install::{Architecture, InstallerType};
use crate::package::PackageSource;
use crate::shared::CatalogId;
use crate::shared::validation::{Validate, ensure_hash, ensure_http_url, ensure_non_empty};
use crate::shared::{ModelError, Version};

/// A validated catalog package entry.
///
/// Catalog packages are source-aware, typed records that are ready for search,
/// selection, and installation workflows. They preserve the source identity and
/// descriptive fields but leave installer discovery to `CatalogInstaller`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPackage {
    /// Canonical catalog id.
    pub id: CatalogId,
    /// Human-readable package name.
    pub name: String,
    /// Parsed semantic version.
    pub version: Version,
    /// Package source.
    pub source: PackageSource,
    /// Optional package summary.
    pub description: Option<String>,
    /// Optional homepage URL.
    pub homepage: Option<String>,
    /// Optional license text.
    pub license: Option<String>,
    /// Optional publisher string.
    pub publisher: Option<String>,
}

/// A validated installer entry associated with a catalog package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogInstaller {
    /// Package id this installer belongs to.
    pub package_id: CatalogId,
    /// Download URL for the installer payload.
    pub url: String,
    /// Expected checksum or empty string when checksumless installs are allowed.
    pub hash: String,
    /// Architecture target for the installer.
    pub arch: Architecture,
    /// Installer format.
    pub kind: InstallerType,
    /// Nested installer format when the installer contains an archive payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested_kind: Option<InstallerType>,
}

impl CatalogPackage {
    /// Validate the package id, source, and version relationship.
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
    /// Validate the installer URL, checksum, and ids.
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

#[cfg(any(test, debug_assertions))]
impl CatalogInstaller {
    pub fn test_builder(package_id: CatalogId, url: &str) -> Self {
        Self {
            package_id,
            url: url.to_string(),
            hash: "abc123".to_string(),
            arch: Architecture::X64,
            kind: InstallerType::Exe,
            nested_kind: None,
        }
    }

    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.hash = hash.into();
        self
    }

    pub fn with_arch(mut self, arch: Architecture) -> Self {
        self.arch = arch;
        self
    }

    pub fn with_kind(mut self, kind: InstallerType) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_nested(mut self, nested_kind: InstallerType) -> Self {
        self.nested_kind = Some(nested_kind);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogInstaller, CatalogPackage};
    use crate::install::{Architecture, InstallerType};
    use crate::package::PackageSource;
    use crate::shared::Version;

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
        let installer = CatalogInstaller::test_builder(
            "winget/Contoso.App".into(),
            "https://example.test/app.exe",
        )
        .with_hash("")
        .with_arch(Architecture::Any)
        .with_kind(InstallerType::Portable);

        assert!(installer.validate().is_ok());
    }

    #[test]
    fn catalog_installer_nested_kind_round_trips_through_serde() {
        let installer = CatalogInstaller::test_builder(
            "winget/Contoso.App".into(),
            "https://example.test/app.zip",
        )
        .with_arch(Architecture::Any)
        .with_kind(InstallerType::Zip)
        .with_nested(InstallerType::Msi)
        .with_hash("sha256:deadbeef");

        let json = serde_json::to_string(&installer).expect("installer should serialize");
        assert!(json.contains("\"nested_kind\":\"msi\""));

        let restored: CatalogInstaller =
            serde_json::from_str(&json).expect("installer should deserialize");

        assert_eq!(restored.nested_kind, Some(InstallerType::Msi));
    }

    #[test]
    fn catalog_installer_defaults_missing_nested_kind_on_deserialize() {
        let json = r#"{
            "package_id":"winget/Contoso.App",
            "url":"https://example.test/app.exe",
            "hash":"sha256:deadbeef",
            "arch":"any",
            "kind":"portable"
        }"#;

        let installer: CatalogInstaller =
            serde_json::from_str(json).expect("installer should deserialize");

        assert_eq!(installer.nested_kind, None);
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
}
