use std::fs::{self, OpenOptions};
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

pub(super) fn validate_target(
    context: &mut ExtractionContext,
    path: &Path,
    destination_dir: &Path,
) -> Result<()> {
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

        if candidate == destination_dir {
            break;
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

    // missing_directories is collected deepest-first, so the first entry is the
    // deepest full path and create_dir_all on it materializes the whole chain.
    if let Some(deepest_missing) = missing_directories.first() {
        fs::create_dir_all(deepest_missing)
            .map_err(|err| FsError::create_directory(deepest_missing, err))?;

        for directory in missing_directories.iter().rev() {
            context.record_directory(directory);
        }
    }

    Ok(())
}

pub(super) fn create_extracted_file(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}
