use std::fs;
use std::path::Path;

use crate::fs::{FsError, Result};

use super::{CachedPath, ExtractionContext, PathInfo};

pub(super) fn inspect_path(path: &Path) -> std::io::Result<PathInfo> {
    let info = winbrew_windows::inspect_path(path)?;
    Ok(PathInfo {
        is_directory: info.is_directory,
        is_reparse_point: info.is_reparse_point,
        hard_link_count: info.hard_link_count,
    })
}

pub(super) fn validate_target(context: &mut ExtractionContext, path: &Path) -> Result<()> {
    let mut current = Some(path);
    let mut is_final_component = true;

    while let Some(candidate) = current {
        match context.inspect_cached(candidate)? {
            CachedPath::Present(info) => {
                if info.is_reparse_point {
                    return Err(FsError::reparse_point(candidate));
                }

                if is_final_component && !info.is_directory && info.hard_link_count > 1 {
                    return Err(FsError::hardlinked_target(candidate));
                }
            }
            CachedPath::Missing => {}
        }

        is_final_component = false;
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
                if info.is_reparse_point {
                    return Err(FsError::reparse_point(candidate));
                }

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
    winbrew_windows::create_extracted_file(path)
}
