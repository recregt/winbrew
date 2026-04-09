use std::fs;
use std::path::Path;

use crate::fs::{FsError, Result};

use super::{CachedPath, ExtractionContext, PathInfo};

pub(super) fn inspect_path(path: &Path) -> std::io::Result<PathInfo> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(PathInfo {
        is_directory: metadata.is_dir(),
        is_reparse_point: false,
        hard_link_count: 1,
    })
}

pub(super) fn validate_target(context: &mut ExtractionContext, path: &Path) -> Result<()> {
    let mut current = Some(path);

    while let Some(candidate) = current {
        match context.inspect_cached(candidate)? {
            CachedPath::Present(info) => {
                if !info.is_directory {
                    return Err(FsError::path_not_directory(candidate));
                }
            }
            CachedPath::Missing => {}
        }

        current = candidate.parent();
    }

    Ok(())
}

pub(super) fn ensure_directory_tree(context: &mut ExtractionContext, path: &Path) -> Result<()> {
    let mut missing_directories = Vec::new();
    let mut current = Some(path);

    while let Some(candidate) = current {
        match context.inspect_cached(candidate)? {
            CachedPath::Present(info) => {
                if !info.is_directory {
                    return Err(FsError::path_not_directory(candidate));
                }

                break;
            }
            CachedPath::Missing => {
                missing_directories.push(candidate.to_path_buf());
                current = candidate.parent();
            }
        }
    }

    for directory in missing_directories.iter().rev() {
        fs::create_dir_all(directory).map_err(|err| FsError::create_directory(directory, err))?;
        context.record_directory(directory);
    }

    Ok(())
}
