use std::fs;
use std::path::Path;

use crate::error::ParserError;

pub use winbrew_models::CatalogMetadata;

pub fn write_metadata(path: &Path, metadata: &CatalogMetadata) -> Result<(), ParserError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let data = serde_json::to_vec_pretty(metadata)?;
    fs::write(path, data)?;
    Ok(())
}
