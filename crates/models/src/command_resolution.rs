use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalog::package::{CanonicalInstallerKey, CatalogInstaller, CatalogPackage};

/// Provenance for a resolved command list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandSource {
    PackageLevel,
    InstallerLevel,
    Moniker,
    SourceId,
    Inferred,
}

/// Confidence assigned to a resolved command set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Low,
    Unresolved,
}

/// Version scope covered by a resolved command set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionScope {
    All,
    Specific(String),
    Latest,
}

/// Why a resolver could not produce a trusted command set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    NoMetadata,
    AmbiguousMatch,
    InferenceTooRisky,
    VersionConflict { versions: Vec<String> },
}

/// Resolver output for command exposure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ResolverResult {
    Resolved {
        commands: Vec<String>,
        confidence: Confidence,
        sources: Vec<CommandSource>,
        version_scope: VersionScope,
        catalog_fingerprint: String,
    },
    Unresolved {
        reason: UnresolvedReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CanonicalFingerprintPayload {
    package_commands: Vec<String>,
    package_bin: Option<String>,
    package_moniker: Option<String>,
    installer_commands: Vec<String>,
    installer_identity: CanonicalFingerprintInstallerIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CanonicalFingerprintInstallerIdentity {
    package_id: String,
    url: String,
    hash: String,
    hash_algorithm: String,
    installer_type: String,
    installer_switches: Option<String>,
    scope: Option<String>,
    arch: String,
    kind: String,
    nested_kind: Option<String>,
}

/// Error returned when a catalog fingerprint cannot be serialized.
pub type CatalogFingerprintError = serde_json::Error;

/// Resolve command exposure from catalog metadata with conservative precedence.
pub fn resolve_command_exposure(
    package: &CatalogPackage,
    installer: &CatalogInstaller,
) -> Result<ResolverResult, CatalogFingerprintError> {
    let package_commands = parse_command_list(package.commands.as_deref())?;
    let installer_commands = parse_command_list(installer.commands.as_deref())?;

    if !package_commands.is_empty() {
        let catalog_fingerprint = catalog_fingerprint(
            &package_commands,
            package.bin.as_deref(),
            package.moniker.as_deref(),
            &installer_commands,
            &installer.canonical_key(),
        )?;

        return Ok(ResolverResult::Resolved {
            commands: package_commands,
            confidence: Confidence::High,
            sources: vec![CommandSource::PackageLevel],
            version_scope: VersionScope::Specific(package.version.to_string()),
            catalog_fingerprint,
        });
    }

    if !installer_commands.is_empty() {
        let catalog_fingerprint = catalog_fingerprint(
            &package_commands,
            package.bin.as_deref(),
            package.moniker.as_deref(),
            &installer_commands,
            &installer.canonical_key(),
        )?;

        return Ok(ResolverResult::Resolved {
            commands: installer_commands,
            confidence: Confidence::Low,
            sources: vec![CommandSource::InstallerLevel],
            version_scope: VersionScope::Specific(package.version.to_string()),
            catalog_fingerprint,
        });
    }

    Ok(ResolverResult::Unresolved {
        reason: UnresolvedReason::NoMetadata,
    })
}

impl ResolverResult {
    /// Return the caller-facing confidence classification.
    pub fn confidence(&self) -> Confidence {
        match self {
            Self::Resolved { confidence, .. } => *confidence,
            Self::Unresolved { .. } => Confidence::Unresolved,
        }
    }
}

/// Compute a stable SHA-256 catalog fingerprint for a resolved exposure decision.
pub fn catalog_fingerprint(
    package_commands: &[String],
    package_bin: Option<&str>,
    package_moniker: Option<&str>,
    installer_commands: &[String],
    installer_identity: &CanonicalInstallerKey,
) -> Result<String, CatalogFingerprintError> {
    let payload = CanonicalFingerprintPayload {
        package_commands: normalize_command_list(package_commands),
        package_bin: normalize_bin_json(package_bin)?,
        package_moniker: normalize_text(package_moniker),
        installer_commands: normalize_command_list(installer_commands),
        installer_identity: CanonicalFingerprintInstallerIdentity::from(installer_identity),
    };

    let bytes = serde_json::to_vec(&payload)?;
    let digest = Sha256::digest(bytes);

    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest.as_slice() {
        write!(&mut encoded, "{:02x}", byte).expect("hex encoding should not fail");
    }

    Ok(format!("sha256:{encoded}"))
}

fn normalize_command_list(values: &[String]) -> Vec<String> {
    let mut normalized = BTreeSet::new();

    for value in values {
        let value = value.trim().to_ascii_lowercase();
        if !value.is_empty() {
            normalized.insert(value);
        }
    }

    normalized.into_iter().collect()
}

fn parse_command_list(raw: Option<&str>) -> Result<Vec<String>, serde_json::Error> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };

    let commands: Vec<String> = serde_json::from_str(raw)?;
    Ok(normalize_command_names(commands))
}

fn normalize_command_names<I, S>(commands: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = BTreeMap::new();

    for command in commands {
        let trimmed = command.as_ref().trim();
        if trimmed.is_empty() {
            continue;
        }

        normalized
            .entry(trimmed.to_ascii_lowercase())
            .or_insert_with(|| trimmed.to_string());
    }

    normalized.into_values().collect()
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalize_bin_json(value: Option<&str>) -> Result<Option<String>, serde_json::Error> {
    let Some(value) = value else {
        return Ok(None);
    };

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = serde_json::from_str::<serde_json::Value>(trimmed)?;
    Ok(Some(serde_json::to_string(&parsed)?))
}

impl From<&CanonicalInstallerKey> for CanonicalFingerprintInstallerIdentity {
    fn from(value: &CanonicalInstallerKey) -> Self {
        Self {
            package_id: value.package_id.clone(),
            url: value.url.clone(),
            hash: value.hash.clone(),
            hash_algorithm: value.hash_algorithm.clone(),
            installer_type: value.installer_type.clone(),
            installer_switches: value.installer_switches.clone(),
            scope: value.scope.clone(),
            arch: value.arch.clone(),
            kind: value.kind.clone(),
            nested_kind: value.nested_kind.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandSource, Confidence, ResolverResult, UnresolvedReason, VersionScope,
        catalog_fingerprint, resolve_command_exposure,
    };
    use crate::catalog::package::{CanonicalInstallerKey, CatalogInstaller, CatalogPackage};
    use crate::shared::Version;

    #[test]
    fn resolves_package_commands_with_high_confidence() {
        let package = CatalogPackage::test_builder(
            "winget/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        )
        .with_moniker("contoso");
        let mut installer = CatalogInstaller::test_builder(
            "winget/Contoso.App".into(),
            "https://example.invalid/app.exe",
        )
        .with_kind(crate::install::InstallerType::Exe);
        let mut package = package;
        package.commands = Some(r#"["Contoso", "contoso"]"#.to_string());
        installer.commands = Some(r#"["Installer"]"#.to_string());

        let resolved = resolve_command_exposure(&package, &installer).expect("resolve commands");

        match resolved {
            ResolverResult::Resolved {
                commands,
                confidence,
                sources,
                version_scope,
                catalog_fingerprint,
            } => {
                assert_eq!(commands, vec!["Contoso".to_string()]);
                assert_eq!(confidence, Confidence::High);
                assert_eq!(sources, vec![CommandSource::PackageLevel]);
                assert_eq!(version_scope, VersionScope::Specific("1.2.3".to_string()));
                assert!(catalog_fingerprint.starts_with("sha256:"));
            }
            other => panic!("expected resolved commands, got {other:?}"),
        }
    }

    #[test]
    fn resolves_installer_commands_when_package_metadata_is_empty() {
        let package = CatalogPackage::test_builder(
            "winget/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        );
        let mut installer = CatalogInstaller::test_builder(
            "winget/Contoso.App".into(),
            "https://example.invalid/app.exe",
        )
        .with_kind(crate::install::InstallerType::Exe);
        installer.commands = Some(r#"["contoso", "Contoso"]"#.to_string());

        let resolved = resolve_command_exposure(&package, &installer).expect("resolve commands");

        match resolved {
            ResolverResult::Resolved {
                commands,
                confidence,
                sources,
                version_scope,
                catalog_fingerprint,
            } => {
                assert_eq!(commands, vec!["contoso".to_string()]);
                assert_eq!(confidence, Confidence::Low);
                assert_eq!(sources, vec![CommandSource::InstallerLevel]);
                assert_eq!(version_scope, VersionScope::Specific("1.2.3".to_string()));
                assert!(catalog_fingerprint.starts_with("sha256:"));
            }
            other => panic!("expected resolved commands, got {other:?}"),
        }
    }

    #[test]
    fn unresolved_when_no_command_metadata_exists() {
        let package = CatalogPackage::test_builder(
            "winget/Contoso.App".into(),
            "Contoso App",
            Version::parse("1.2.3").expect("version should parse"),
        )
        .with_moniker("contoso");
        let installer = CatalogInstaller::test_builder(
            "winget/Contoso.App".into(),
            "https://example.invalid/app.exe",
        )
        .with_kind(crate::install::InstallerType::Exe);

        let resolved = resolve_command_exposure(&package, &installer).expect("resolve commands");

        assert_eq!(
            resolved,
            ResolverResult::Unresolved {
                reason: UnresolvedReason::NoMetadata,
            }
        );
        assert_eq!(resolved.confidence(), Confidence::Unresolved);
    }

    #[test]
    fn resolver_result_round_trips() {
        let result = ResolverResult::Resolved {
            commands: vec!["alacritty".to_string()],
            confidence: Confidence::High,
            sources: vec![CommandSource::PackageLevel, CommandSource::InstallerLevel],
            version_scope: VersionScope::Latest,
            catalog_fingerprint: "sha256:deadbeef".to_string(),
        };

        let json = serde_json::to_string(&result).expect("serialize result");
        let restored: ResolverResult = serde_json::from_str(&json).expect("deserialize result");

        assert_eq!(restored, result);
        assert_eq!(restored.confidence(), Confidence::High);
    }

    #[test]
    fn unresolved_result_round_trips() {
        let result = ResolverResult::Unresolved {
            reason: UnresolvedReason::VersionConflict {
                versions: vec!["1.0.0".to_string(), "2.0.0".to_string()],
            },
        };

        let json = serde_json::to_string(&result).expect("serialize result");
        let restored: ResolverResult = serde_json::from_str(&json).expect("deserialize result");

        assert_eq!(restored, result);
        assert_eq!(restored.confidence(), Confidence::Unresolved);
    }

    #[test]
    fn catalog_fingerprint_is_stable_for_normalized_inputs() {
        let identity = CanonicalInstallerKey {
            package_id: "winget/Contoso.App".to_string(),
            url: "https://example.invalid/app.exe".to_string(),
            hash: "sha256:deadbeef".to_string(),
            hash_algorithm: "sha256".to_string(),
            installer_type: "portable".to_string(),
            installer_switches: Some("/S".to_string()),
            scope: Some("machine".to_string()),
            arch: "x64".to_string(),
            kind: "portable".to_string(),
            nested_kind: None,
        };

        let first = catalog_fingerprint(
            &["Alacritty".to_string(), "alacritty".to_string()],
            Some(r#"["bin\\tool.exe"]"#),
            Some("Alacritty"),
            &["ALACRITTY".to_string()],
            &identity,
        )
        .expect("fingerprint");

        let second = catalog_fingerprint(
            &["alacritty".to_string()],
            Some(" [\n  \"bin\\\\tool.exe\"\n] "),
            Some("alacritty"),
            &["alacritty".to_string()],
            &identity,
        )
        .expect("fingerprint");

        assert_eq!(first, second);
        assert!(first.starts_with("sha256:"));
    }
}
