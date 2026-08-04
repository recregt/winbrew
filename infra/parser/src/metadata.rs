use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ParserError;

pub use winbrew_models::catalog::metadata::CatalogMetadata;

pub fn write_metadata(path: &Path, metadata: &CatalogMetadata) -> Result<(), ParserError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let data = serde_json::to_vec_pretty(metadata)?;

    // Write to a sibling temp file and rename into place so a reader (or a
    // crash mid-write) never observes a truncated metadata.json.
    let temp_path = sibling_temp_path(path);
    fs::write(&temp_path, &data)?;
    fs::rename(&temp_path, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp_path);
    })?;

    Ok(())
}

fn sibling_temp_path(path: &Path) -> PathBuf {
    let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
    temp_name.push(".tmp");
    path.with_file_name(temp_name)
}

#[cfg(test)]
mod tests {
    use super::{CatalogMetadata, write_metadata};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(suffix: &str) -> PathBuf {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "winbrew-parser-metadata-{}-{suffix}-{unique_id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn write_metadata_leaves_no_temp_file_behind() {
        let dir = temp_dir("no-tmp-leftover");
        let path = dir.join("metadata.json");
        let metadata = CatalogMetadata::build_from_counts(0, BTreeMap::new(), "sha256:abc".into());

        write_metadata(&path, &metadata).expect("write metadata");

        assert!(path.exists());
        assert!(!dir.join("metadata.json.tmp").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_metadata_overwrites_existing_file_atomically() {
        let dir = temp_dir("overwrite");
        let path = dir.join("metadata.json");
        fs::write(&path, b"stale").expect("seed stale metadata");

        let metadata = CatalogMetadata::build_from_counts(1, BTreeMap::new(), "sha256:def".into());
        write_metadata(&path, &metadata).expect("write metadata");

        let written = fs::read_to_string(&path).expect("read metadata");
        assert!(written.contains("sha256:def"));

        fs::remove_dir_all(&dir).ok();
    }
}
