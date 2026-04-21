use std::collections::BTreeSet;
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalog::package::CanonicalInstallerKey;

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
        catalog_fingerprint,
    };
    use crate::catalog::package::CanonicalInstallerKey;

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
