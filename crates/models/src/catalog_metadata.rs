use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::ModelError;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogMetadata {
    pub schema_version: u32,
    pub generated_at_unix: u64,
    pub current_hash: String,
    #[serde(default)]
    pub previous_hash: String,
    pub package_count: usize,
    pub source_counts: BTreeMap<String, usize>,
}

impl CatalogMetadata {
    pub fn build_from_counts(
        package_count: usize,
        source_counts: BTreeMap<String, usize>,
        current_hash: String,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            generated_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            current_hash,
            previous_hash: String::default(),
            package_count,
            source_counts,
        }
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ModelError::invalid_contract(
                "catalog_metadata.schema_version",
                format!(
                    "unsupported catalog metadata schema version: expected {SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            ));
        }

        if self.current_hash.trim().is_empty() {
            return Err(ModelError::invalid_contract(
                "catalog_metadata.current_hash",
                "current_hash cannot be empty",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CatalogMetadata;
    use std::collections::BTreeMap;

    #[test]
    fn builds_metadata_with_schema_version() {
        let metadata = CatalogMetadata::build_from_counts(
            2,
            BTreeMap::from([(String::from("scoop"), 1)]),
            String::from("sha256:abc"),
        );

        assert_eq!(metadata.schema_version, 1);
        assert_eq!(metadata.package_count, 2);
        assert_eq!(metadata.source_counts.get("scoop"), Some(&1));
    }
}
