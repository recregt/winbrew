use serde::{Deserialize, Serialize};

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
}

impl ScoopStreamEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCOOP_STREAM_SCHEMA_VERSION {
            return Err(format!(
                "unsupported scoop stream schema version: {}",
                self.schema_version
            ));
        }
        if self.source != SCOOP_STREAM_SOURCE {
            return Err(format!("unexpected scoop stream source: {}", self.source));
        }
        if self.kind != SCOOP_STREAM_KIND {
            return Err(format!("unexpected scoop stream kind: {}", self.kind));
        }

        Ok(())
    }
}
