use serde::{Deserialize, Serialize};

use winbrew_models::shared::error::ModelError;

const SCOOP_STREAM_SCHEMA_VERSION: u32 = 1;
const SCOOP_STREAM_SOURCE: &str = "scoop";
const SCOOP_STREAM_KIND: &str = "package";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoopStreamEnvelope {
    pub schema_version: u32,
    pub source: String,
    pub kind: String,
    pub payload: RawFetchedPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawFetchedPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moniker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<serde_json::Value>,
    #[serde(default)]
    pub installers: Vec<RawFetchedInstaller>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawFetchedInstaller {
    pub url: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub arch: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(
        rename = "NestedInstallerType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub nested_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl ScoopStreamEnvelope {
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.schema_version != SCOOP_STREAM_SCHEMA_VERSION {
            return Err(ModelError::invalid_contract(
                "scoop_stream.schema_version",
                format!(
                    "unsupported scoop stream schema version: expected {SCOOP_STREAM_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            ));
        }

        if self.source != SCOOP_STREAM_SOURCE {
            return Err(ModelError::invalid_contract(
                "scoop_stream.source",
                format!("expected {SCOOP_STREAM_SOURCE}, got {}", self.source),
            ));
        }

        if self.kind != SCOOP_STREAM_KIND {
            return Err(ModelError::invalid_contract(
                "scoop_stream.kind",
                format!("expected {SCOOP_STREAM_KIND}, got {}", self.kind),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{RawFetchedInstaller, RawFetchedPackage, ScoopStreamEnvelope};

    #[test]
    fn validates_expected_envelope() {
        let envelope = ScoopStreamEnvelope {
            schema_version: 1,
            source: "scoop".to_string(),
            kind: "package".to_string(),
            payload: RawFetchedPackage {
                id: "scoop/main/example".to_string(),
                name: "example".to_string(),
                version: "1.2.3".to_string(),
                description: None,
                homepage: None,
                license: None,
                publisher: None,
                locale: None,
                moniker: None,
                tags: None,
                bin: None,
                installers: vec![RawFetchedInstaller {
                    url: "https://example.invalid/example.zip".to_string(),
                    hash: "sha256:deadbeef".to_string(),
                    arch: "x64".to_string(),
                    kind: "portable".to_string(),
                    nested_kind: None,
                    scope: None,
                }],
            },
        };

        assert!(envelope.validate().is_ok());
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let envelope = ScoopStreamEnvelope {
            schema_version: 2,
            source: "scoop".to_string(),
            kind: "package".to_string(),
            payload: RawFetchedPackage {
                id: "scoop/main/example".to_string(),
                name: "example".to_string(),
                version: "1.2.3".to_string(),
                description: None,
                homepage: None,
                license: None,
                publisher: None,
                locale: None,
                moniker: None,
                tags: None,
                bin: None,
                installers: Vec::new(),
            },
        };

        let err = envelope
            .validate()
            .expect_err("schema version mismatch should fail");
        assert!(
            err.to_string()
                .contains("unsupported scoop stream schema version")
        );
    }

    #[test]
    fn deserializes_winget_nested_installer_type_field() {
        let installer: RawFetchedInstaller = serde_json::from_str(
            r#"{
                "url": "https://example.invalid/app.zip",
                "hash": "sha256:deadbeef",
                "arch": "x64",
                "type": "zip",
                "NestedInstallerType": "msi",
                "scope": "user"
            }"#,
        )
        .expect("installer should deserialize");

        assert_eq!(installer.nested_kind.as_deref(), Some("msi"));
        assert_eq!(installer.scope.as_deref(), Some("user"));
    }
}
