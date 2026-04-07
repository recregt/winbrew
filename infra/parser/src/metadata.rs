use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::ParserError;
use crate::parser::ParsedPackage;

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
    pub fn build(records: &[ParsedPackage], current_hash: String) -> Self {
        let mut source_counts = BTreeMap::new();
        for record in records {
            let source = record.package.source.as_str();
            if let Some(count) = source_counts.get_mut(source) {
                *count += 1;
            } else {
                source_counts.insert(source.to_string(), 1);
            }
        }

        Self {
            schema_version: SCHEMA_VERSION,
            generated_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            current_hash,
            previous_hash: String::new(),
            package_count: records.len(),
            source_counts,
        }
    }

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
            previous_hash: String::new(),
            package_count,
            source_counts,
        }
    }
}

pub fn write_metadata(path: &Path, metadata: &CatalogMetadata) -> Result<(), ParserError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let data = serde_json::to_vec_pretty(metadata)?;
    fs::write(path, data)?;
    Ok(())
}
