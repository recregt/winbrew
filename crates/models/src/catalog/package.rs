use serde::{Deserialize, Serialize};

use crate::catalog::installer_type::CatalogInstallerType;
use crate::install::{Architecture, InstallerType};
use crate::package::{PackageId, PackageSource};
use crate::shared::CatalogId;
use crate::shared::validation::{Validate, ensure_hash, ensure_http_url, ensure_non_empty};
use crate::shared::{HashAlgorithm, ModelError, Version};

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
    /// Optional namespace or bucket within the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Source-local identifier for the package.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_id: String,
    /// When the catalog row was first written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// When the catalog row was last updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
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
    /// Checksum algorithm used to verify the installer.
    pub hash_algorithm: HashAlgorithm,
    /// Normalized installer family used for catalog browsing and filtering.
    pub installer_type: CatalogInstallerType,
    /// Silent-install or package-manager switches when the source provides them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer_switches: Option<String>,
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
        ensure_non_empty("catalog_package.source_id", &self.source_id)?;
        if let Some(namespace) = self.namespace.as_deref() {
            ensure_non_empty("catalog_package.namespace", namespace)?;
        }
        self.version.validate()?;

        let package_id = PackageId::parse(self.id.as_ref())?;
        let expected_source = package_id.source();
        if self.source != expected_source {
            return Err(ModelError::source_mismatch(
                "catalog_package.source",
                expected_source.as_str(),
                self.source.as_str(),
            ));
        }

        if self.namespace.as_deref() != package_id.namespace() {
            return Err(ModelError::invalid_contract(
                "catalog_package.namespace",
                format!(
                    "expected {:?}, got {:?}",
                    package_id.namespace(),
                    self.namespace.as_deref()
                ),
            ));
        }

        if self.source_id != package_id.source_id() {
            return Err(ModelError::invalid_contract(
                "catalog_package.source_id",
                format!(
                    "expected {}, got {}",
                    package_id.source_id(),
                    self.source_id
                ),
            ));
        }

        if let Some(created_at) = self.created_at.as_deref() {
            ensure_non_empty("catalog_package.created_at", created_at)?;
        }

        if let Some(updated_at) = self.updated_at.as_deref() {
            ensure_non_empty("catalog_package.updated_at", updated_at)?;
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

            if let Some(expected_algorithm) = HashAlgorithm::detect(&self.hash)
                && expected_algorithm != self.hash_algorithm
            {
                return Err(ModelError::invalid_contract(
                    "catalog_installer.hash_algorithm",
                    format!(
                        "expected {}, got {}",
                        expected_algorithm.as_str(),
                        self.hash_algorithm.as_str()
                    ),
                ));
            }
        }

        if let Some(installer_switches) = self.installer_switches.as_deref() {
            ensure_non_empty("catalog_installer.installer_switches", installer_switches)?;
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
            hash_algorithm: HashAlgorithm::Sha256,
            installer_type: CatalogInstallerType::Unknown,
            installer_switches: None,
            arch: Architecture::X64,
            kind: InstallerType::Exe,
            nested_kind: None,
        }
    }

    pub fn with_installer_type(mut self, installer_type: CatalogInstallerType) -> Self {
        self.installer_type = installer_type;
        self
    }

    pub fn with_installer_switches(mut self, installer_switches: impl Into<String>) -> Self {
        self.installer_switches = Some(installer_switches.into());
        self
    }

    pub fn with_hash_algorithm(mut self, hash_algorithm: HashAlgorithm) -> Self {
        self.hash_algorithm = hash_algorithm;
        self
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

#[cfg(any(test, debug_assertions))]
impl CatalogPackage {
    pub fn test_builder(id: CatalogId, name: &str, version: Version) -> Self {
        let package_id = PackageId::parse(id.as_ref()).expect("catalog id should parse");

        Self {
            id,
            name: name.to_string(),
            version,
            source: package_id.source(),
            namespace: package_id.namespace().map(str::to_string),
            source_id: package_id.source_id().to_string(),
            created_at: None,
            updated_at: None,
            description: None,
            homepage: None,
            license: None,
            publisher: None,
        }
    }

    pub fn with_source(mut self, source: PackageSource) -> Self {
        self.source = source;
        self
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn without_namespace(mut self) -> Self {
        self.namespace = None;
        self
    }

    pub fn with_source_id(mut self, source_id: impl Into<String>) -> Self {
        self.source_id = source_id.into();
        self
    }

    pub fn with_created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = Some(created_at.into());
        self
    }

    pub fn with_updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = Some(updated_at.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_homepage(mut self, homepage: impl Into<String>) -> Self {
        self.homepage = Some(homepage.into());
        self
    }

    pub fn with_license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    pub fn with_publisher(mut self, publisher: impl Into<String>) -> Self {
        self.publisher = Some(publisher.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogInstaller, CatalogPackage};
    use crate::catalog::installer_type::CatalogInstallerType;
    use crate::install::{Architecture, InstallerType};
    use crate::package::PackageSource;
    use crate::shared::{HashAlgorithm, Version};

    #[test]
    fn rejects_source_mismatch() {
        let package = CatalogPackage::test_builder(
            "winget/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        )
        .with_source(PackageSource::Scoop);

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
        .with_hash("deadbeef")
        .with_hash_algorithm(HashAlgorithm::Sha256)
        .with_installer_type(CatalogInstallerType::Zip)
        .with_installer_switches("/S");

        let json = serde_json::to_string(&installer).expect("installer should serialize");
        assert!(json.contains("\"nested_kind\":\"msi\""));
        assert!(json.contains("\"hash_algorithm\":\"sha256\""));
        assert!(json.contains("\"installer_type\":\"zip\""));
        assert!(json.contains("\"installer_switches\":\"/S\""));

        let restored: CatalogInstaller =
            serde_json::from_str(&json).expect("installer should deserialize");

        assert_eq!(restored.nested_kind, Some(InstallerType::Msi));
        assert_eq!(restored.hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(restored.installer_type, CatalogInstallerType::Zip);
        assert_eq!(restored.installer_switches.as_deref(), Some("/S"));
    }

    #[test]
    fn catalog_installer_deserializes_without_nested_kind() {
        let json = r#"{
            "package_id":"winget/Contoso.App",
            "url":"https://example.test/app.exe",
            "hash":"sha256:deadbeef",
            "hash_algorithm":"sha256",
            "installer_type":"unknown",
            "arch":"any",
            "kind":"portable"
        }"#;

        let installer: CatalogInstaller =
            serde_json::from_str(json).expect("installer should deserialize");

        assert_eq!(installer.nested_kind, None);
        assert_eq!(installer.hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(installer.installer_type, CatalogInstallerType::Unknown);
        assert_eq!(installer.installer_switches, None);
    }

    #[test]
    fn catalog_package_round_trips_through_serde() {
        let package = CatalogPackage::test_builder(
            "scoop/main/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        )
        .with_description("Example package")
        .with_created_at("2026-04-14 12:00:00")
        .with_updated_at("2026-04-14 12:34:56")
        .with_publisher("Contoso Ltd.");

        let json = serde_json::to_string(&package).expect("package should serialize");
        let restored: CatalogPackage =
            serde_json::from_str(&json).expect("package should deserialize");

        assert_eq!(restored.id, package.id);
        assert_eq!(restored.source, package.source);
        assert_eq!(restored.namespace, package.namespace);
        assert_eq!(restored.source_id, package.source_id);
        assert_eq!(restored.created_at, package.created_at);
        assert_eq!(restored.updated_at, package.updated_at);
        assert_eq!(restored.version, package.version);
        assert_eq!(restored.publisher, package.publisher);
    }
}
