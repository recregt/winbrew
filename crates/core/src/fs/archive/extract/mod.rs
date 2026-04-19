//! Archive extraction facade.
//! This module provides a high-level API for extracting archive payloads while
//! preserving the ZIP security and rollback behavior WinBrew already relies on.

mod cleanup;
mod context;
mod engine;
mod gzip;
mod limits;
mod sevenz;
mod tar;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use cleanup::ExtractionCleanup;
pub(crate) use context::ExtractionContext;
pub(crate) use limits::ExtractionLimits;
pub(crate) use types::{CachedPath, PathInfo};

use super::ArchiveKind;
use crate::fs::{FsError, Result};
use std::path::Path;

type BoxedResult<T> = std::result::Result<T, Box<FsError>>;

#[cfg(not(windows))]
use super::platform::PortablePlatform as DefaultPlatform;
#[cfg(windows)]
use super::platform::WindowsPlatform as DefaultPlatform;

/// Extracts an archive into `destination_dir`, rejecting entries with invalid paths.
pub fn extract_archive(
    archive_kind: ArchiveKind,
    archive_path: &Path,
    destination_dir: &Path,
) -> BoxedResult<()> {
    match archive_kind {
        ArchiveKind::Zip => extract_zip_archive_with_limits(
            archive_path,
            destination_dir,
            ExtractionLimits::default(),
        )
        .map_err(Box::new),
        ArchiveKind::Gzip => {
            gzip::extract_gzip_archive(archive_path, destination_dir).map_err(Box::new)
        }
        ArchiveKind::SevenZip => {
            sevenz::extract_sevenz(archive_path, destination_dir).map_err(Box::new)
        }
        ArchiveKind::Tar => tar::extract_tar_archive_with_platform::<DefaultPlatform>(
            archive_path,
            destination_dir,
            ExtractionLimits::default(),
        )
        .map_err(Box::new),
        _ => Err(Box::new(FsError::archive_backend_unavailable(
            archive_kind.as_str(),
        ))),
    }
}

/// Extracts a ZIP archive into the destination directory.
///
/// The extraction target is validated so the archive cannot be unpacked through
/// an existing reparse-point ancestor, and symlink entries are refused.
pub fn extract_zip_archive(zip_path: &Path, destination_dir: &Path) -> BoxedResult<()> {
    extract_archive(ArchiveKind::Zip, zip_path, destination_dir)
}

fn extract_zip_archive_with_limits(
    zip_path: &Path,
    destination_dir: &Path,
    limits: ExtractionLimits,
) -> Result<()> {
    engine::extract_zip_archive_with_platform::<DefaultPlatform>(zip_path, destination_dir, limits)
}
