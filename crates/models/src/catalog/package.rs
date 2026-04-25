use std::collections::BTreeSet;

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
    /// Optional package metadata locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Optional package moniker or alias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moniker: Option<String>,
    /// Optional package platform metadata encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Optional package commands encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<String>,
    /// Optional package protocols encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<String>,
    /// Optional package file extensions encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_extensions: Option<String>,
    /// Optional package capabilities encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<String>,
    /// Optional package search tags encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Optional package bin metadata encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
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
    /// Optional installer platform metadata encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Optional installer commands encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<String>,
    /// Optional installer protocols encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<String>,
    /// Optional installer file extensions encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_extensions: Option<String>,
    /// Optional installer capabilities encoded as JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<String>,
    /// Architecture target for the installer.
    pub arch: Architecture,
    /// Raw installer format used by the engine-facing model, distinct from `installer_type`.
    pub kind: InstallerType,
    /// Nested installer format when the installer contains an archive payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested_kind: Option<InstallerType>,
    /// Optional install scope reported by the source, usually `user` or `machine` for Winget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Canonical identity for a catalog installer row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalInstallerKey {
    /// Package id this installer belongs to.
    pub package_id: String,
    /// Download URL for the installer payload.
    pub url: String,
    /// Expected checksum or empty string when checksumless installs are allowed.
    pub hash: String,
    /// Checksum algorithm used to verify the installer.
    pub hash_algorithm: String,
    /// Normalized installer family used for catalog browsing and filtering.
    pub installer_type: String,
    /// Silent-install or package-manager switches when the source provides them.
    pub installer_switches: Option<String>,
    /// Optional install scope reported by the source.
    pub scope: Option<String>,
    /// Architecture target for the installer.
    pub arch: String,
    /// Raw installer format used by the engine-facing model, distinct from `installer_type`.
    pub kind: String,
    /// Nested installer format when the installer contains an archive payload.
    pub nested_kind: Option<String>,
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

        if let Some(locale) = self.locale.as_deref() {
            ensure_non_empty("catalog_package.locale", locale)?;
        } else if self.source == PackageSource::Winget {
            return Err(ModelError::invalid_contract(
                "catalog_package.locale",
                "winget packages require a locale",
            ));
        }

        if let Some(moniker) = self.moniker.as_deref() {
            ensure_non_empty("catalog_package.moniker", moniker)?;
        }

        if let Some(platform) = self.platform.as_deref() {
            ensure_non_empty("catalog_package.platform", platform)?;
        }

        if let Some(commands) = self.commands.as_deref() {
            ensure_non_empty("catalog_package.commands", commands)?;
        }

        if let Some(protocols) = self.protocols.as_deref() {
            ensure_non_empty("catalog_package.protocols", protocols)?;
        }

        if let Some(file_extensions) = self.file_extensions.as_deref() {
            ensure_non_empty("catalog_package.file_extensions", file_extensions)?;
        }

        if let Some(capabilities) = self.capabilities.as_deref() {
            ensure_non_empty("catalog_package.capabilities", capabilities)?;
        }

        if let Some(tags) = self.tags.as_deref() {
            ensure_non_empty("catalog_package.tags", tags)?;
        }

        if let Some(bin) = self.bin.as_deref() {
            ensure_non_empty("catalog_package.bin", bin)?;
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

        if let Some(platform) = self.platform.as_deref() {
            ensure_non_empty("catalog_installer.platform", platform)?;
        }

        if let Some(commands) = self.commands.as_deref() {
            ensure_non_empty("catalog_installer.commands", commands)?;
        }

        if let Some(protocols) = self.protocols.as_deref() {
            ensure_non_empty("catalog_installer.protocols", protocols)?;
        }

        if let Some(file_extensions) = self.file_extensions.as_deref() {
            ensure_non_empty("catalog_installer.file_extensions", file_extensions)?;
        }

        if let Some(capabilities) = self.capabilities.as_deref() {
            ensure_non_empty("catalog_installer.capabilities", capabilities)?;
        }

        if let Some(scope) = self.scope.as_deref() {
            let normalized_scope = scope.trim().to_ascii_lowercase();
            if !matches!(normalized_scope.as_str(), "user" | "machine") {
                return Err(ModelError::invalid_contract(
                    "catalog_installer.scope",
                    format!("expected user or machine, got {scope}"),
                ));
            }
        }

        Ok(())
    }

    /// Return the canonical identity used to deduplicate installer rows.
    pub fn canonical_key(&self) -> CanonicalInstallerKey {
        CanonicalInstallerKey {
            package_id: self.package_id.to_string(),
            url: self.url.clone(),
            hash: self.hash.clone(),
            hash_algorithm: self.hash_algorithm.as_str().to_string(),
            installer_type: self.installer_type.as_str().to_string(),
            installer_switches: self.installer_switches.clone(),
            scope: self.scope.clone(),
            arch: self.arch.as_str().to_string(),
            kind: self.kind.to_string(),
            nested_kind: self.nested_kind.map(|kind| kind.to_string()),
        }
    }

    /// Merge metadata-only fields from another installer that shares the same canonical key.
    pub fn merge_metadata_from(&mut self, other: &Self) -> Result<(), ModelError> {
        if self.canonical_key() != other.canonical_key() {
            return Err(ModelError::invalid_contract(
                "catalog_installer.merge",
                "cannot merge installers with different canonical keys",
            ));
        }

        self.platform = merge_json_text_array(
            self.platform.take(),
            other.platform.as_deref(),
            "catalog_installer.platform",
        )?;
        self.commands = merge_json_text_array(
            self.commands.take(),
            other.commands.as_deref(),
            "catalog_installer.commands",
        )?;
        self.protocols = merge_json_text_array(
            self.protocols.take(),
            other.protocols.as_deref(),
            "catalog_installer.protocols",
        )?;
        self.file_extensions = merge_json_text_array(
            self.file_extensions.take(),
            other.file_extensions.as_deref(),
            "catalog_installer.file_extensions",
        )?;
        self.capabilities = merge_json_text_array(
            self.capabilities.take(),
            other.capabilities.as_deref(),
            "catalog_installer.capabilities",
        )?;

        Ok(())
    }
}

impl Validate for CatalogInstaller {
    fn validate(&self) -> Result<(), ModelError> {
        CatalogInstaller::validate(self)
    }
}

fn merge_json_text_array(
    left: Option<String>,
    right: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, ModelError> {
    let mut values = BTreeSet::new();

    if let Some(value) = left.as_deref() {
        collect_json_text_array(field, value, &mut values)?;
    }

    if let Some(value) = right {
        collect_json_text_array(field, value, &mut values)?;
    }

    if values.is_empty() {
        return Ok(None);
    }

    let merged: Vec<String> = values.into_iter().collect();
    let json = serde_json::to_string(&merged)
        .map_err(|err| ModelError::invalid_contract(field, err.to_string()))?;

    Ok(Some(json))
}

fn collect_json_text_array(
    field: &'static str,
    json: &str,
    values: &mut BTreeSet<String>,
) -> Result<(), ModelError> {
    let entries: Vec<String> = serde_json::from_str(json)
        .map_err(|err| ModelError::invalid_contract(field, err.to_string()))?;

    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }

        values.insert(trimmed.to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CatalogInstaller, CatalogPackage};
    use crate::catalog::installer_type::CatalogInstallerType;
    use crate::install::{Architecture, InstallerType};
    use crate::package::PackageId;
    use crate::package::PackageSource;
    use crate::shared::CatalogId;
    use crate::shared::{HashAlgorithm, Version};

    fn catalog_installer(package_id: CatalogId, url: &str) -> CatalogInstaller {
        CatalogInstaller {
            package_id,
            url: url.to_string(),
            hash: "abc123".to_string(),
            hash_algorithm: HashAlgorithm::Sha256,
            installer_type: CatalogInstallerType::Unknown,
            installer_switches: None,
            platform: None,
            commands: None,
            protocols: None,
            file_extensions: None,
            capabilities: None,
            arch: Architecture::X64,
            kind: InstallerType::Exe,
            nested_kind: None,
            scope: None,
        }
    }

    fn catalog_package(id: CatalogId, name: &str, version: Version) -> CatalogPackage {
        let package_id = PackageId::parse(id.as_ref()).expect("catalog id should parse");

        CatalogPackage {
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

    #[test]
    fn rejects_source_mismatch() {
        let mut package = catalog_package(
            "winget/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        );
        package.source = PackageSource::Scoop;

        let err = package.validate().expect_err("source mismatch should fail");

        assert!(err.to_string().contains("source mismatch"));
    }

    #[test]
    fn validates_checksumless_catalog_installer() {
        let installer =
            catalog_installer("winget/Contoso.App".into(), "https://example.test/app.exe");
        let installer = CatalogInstaller {
            hash: "".to_string(),
            arch: Architecture::Any,
            kind: InstallerType::Portable,
            ..installer
        };

        assert!(installer.validate().is_ok());
    }

    #[test]
    fn catalog_installer_nested_kind_round_trips_through_serde() {
        let mut installer =
            catalog_installer("winget/Contoso.App".into(), "https://example.test/app.zip");
        installer.arch = Architecture::Any;
        installer.kind = InstallerType::Zip;
        installer.nested_kind = Some(InstallerType::Msi);
        installer.scope = Some("user".to_string());
        installer.hash = "deadbeef".to_string();
        installer.hash_algorithm = HashAlgorithm::Sha256;
        installer.installer_type = CatalogInstallerType::Zip;
        installer.installer_switches = Some("/S".to_string());

        let json = serde_json::to_string(&installer).expect("installer should serialize");
        assert!(json.contains("\"nested_kind\":\"msi\""));
        assert!(json.contains("\"hash_algorithm\":\"sha256\""));
        assert!(json.contains("\"installer_type\":\"zip\""));
        assert!(json.contains("\"installer_switches\":\"/S\""));
        assert!(json.contains("\"scope\":\"user\""));

        let restored: CatalogInstaller =
            serde_json::from_str(&json).expect("installer should deserialize");

        assert_eq!(restored.nested_kind, Some(InstallerType::Msi));
        assert_eq!(restored.hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(restored.installer_type, CatalogInstallerType::Zip);
        assert_eq!(restored.installer_switches.as_deref(), Some("/S"));
        assert_eq!(restored.scope.as_deref(), Some("user"));
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
    fn canonical_key_distinguishes_nested_kind_presence() {
        let mut base =
            catalog_installer("winget/Contoso.App".into(), "https://example.test/app.zip");
        base.hash = "sha256:deadbeef".to_string();
        base.hash_algorithm = HashAlgorithm::Sha256;
        base.installer_type = CatalogInstallerType::Zip;
        base.arch = Architecture::Any;
        base.kind = InstallerType::Zip;

        let mut nested = base.clone();
        nested.nested_kind = Some(InstallerType::Msi);

        assert_ne!(base.canonical_key(), nested.canonical_key());
    }

    #[test]
    fn merge_metadata_unions_arrays_deterministically() {
        let mut left =
            catalog_installer("winget/Contoso.App".into(), "https://example.test/app.zip");
        left.hash = "sha256:deadbeef".to_string();
        left.hash_algorithm = HashAlgorithm::Sha256;
        left.installer_type = CatalogInstallerType::Zip;
        left.arch = Architecture::Any;
        left.kind = InstallerType::Zip;
        left.nested_kind = Some(InstallerType::Msi);
        left.platform = Some("[\"Windows.Server\", \"Windows.Desktop\"]".to_string());
        left.commands = Some("[\"contoso\"]".to_string());
        left.protocols = Some("[\"contoso-protocol\"]".to_string());
        left.file_extensions = Some("[\".exe\"]".to_string());
        left.capabilities = Some("[\"internetClient\"]".to_string());

        let mut right = left.clone();
        right.platform = Some("[\"Windows.Desktop\", \"Windows.LTSC\"]".to_string());
        right.commands = Some("[\"contoso-server\", \"contoso\"]".to_string());
        right.protocols = Some("[\"contoso-protocol\", \"contoso-shell\"]".to_string());
        right.file_extensions = Some("[\".msi\", \".exe\"]".to_string());
        right.capabilities = Some("[\"internetClient\", \"internetClientServer\"]".to_string());

        left.merge_metadata_from(&right)
            .expect("merge should succeed");

        assert_eq!(
            left.platform.as_deref(),
            Some("[\"Windows.Desktop\",\"Windows.LTSC\",\"Windows.Server\"]")
        );
        assert_eq!(
            left.commands.as_deref(),
            Some("[\"contoso\",\"contoso-server\"]")
        );
        assert_eq!(
            left.protocols.as_deref(),
            Some("[\"contoso-protocol\",\"contoso-shell\"]")
        );
        assert_eq!(left.file_extensions.as_deref(), Some("[\".exe\",\".msi\"]"));
        assert_eq!(
            left.capabilities.as_deref(),
            Some("[\"internetClient\",\"internetClientServer\"]")
        );
    }

    #[test]
    fn merge_metadata_preserves_present_side_when_other_is_missing() {
        let mut left =
            catalog_installer("winget/Contoso.App".into(), "https://example.test/app.zip");
        left.hash = "sha256:deadbeef".to_string();
        left.hash_algorithm = HashAlgorithm::Sha256;
        left.installer_type = CatalogInstallerType::Zip;
        left.arch = Architecture::Any;
        left.kind = InstallerType::Zip;
        left.nested_kind = None;

        let mut right = left.clone();
        right.platform = Some("[\"Windows.Desktop\"]".to_string());
        right.commands = None;
        right.protocols = Some("[\"contoso-protocol\"]".to_string());
        right.file_extensions = None;
        right.capabilities = None;

        left.merge_metadata_from(&right)
            .expect("merge should succeed");

        assert_eq!(left.platform.as_deref(), Some("[\"Windows.Desktop\"]"));
        assert_eq!(left.commands, None);
        assert_eq!(left.protocols.as_deref(), Some("[\"contoso-protocol\"]"));
        assert_eq!(left.file_extensions, None);
        assert_eq!(left.capabilities, None);
    }

    #[test]
    fn catalog_package_round_trips_through_serde() {
        let mut package = catalog_package(
            "scoop/main/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        );
        package.description = Some("Example package".to_string());
        package.created_at = Some("2026-04-14 12:00:00".to_string());
        package.updated_at = Some("2026-04-14 12:34:56".to_string());
        package.publisher = Some("Contoso Ltd.".to_string());
        package.locale = Some("en-US".to_string());
        package.moniker = Some("contoso".to_string());
        package.tags = Some("[\"utility\"]".to_string());
        package.bin = Some("[\"tool.exe\"]".to_string());

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
        assert_eq!(restored.locale, package.locale);
        assert_eq!(restored.moniker, package.moniker);
        assert_eq!(restored.tags, package.tags);
        assert_eq!(restored.bin, package.bin);
    }

    #[test]
    fn winget_packages_require_locale() {
        let package = catalog_package(
            "winget/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        );

        let err = package
            .validate()
            .expect_err("winget package should require locale");

        assert!(err.to_string().contains("require a locale"));
    }
}
