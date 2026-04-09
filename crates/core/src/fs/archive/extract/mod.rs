//! ZIP archive extraction facade.

#![allow(clippy::result_large_err)]

mod cleanup;
mod context;
mod engine;
mod limits;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use cleanup::ExtractionCleanup;
pub(crate) use context::ExtractionContext;
pub(crate) use limits::ExtractionLimits;
pub(crate) use types::{CachedPath, PathInfo};

use crate::fs::Result;
use std::path::Path;

#[cfg(not(windows))]
use super::platform::PortablePlatform as DefaultPlatform;
#[cfg(windows)]
use super::platform::WindowsPlatform as DefaultPlatform;

/// Extracts `zip_path` into `destination_dir`, rejecting entries with invalid paths.
///
/// The extraction target is validated so the archive cannot be unpacked through
/// an existing reparse-point ancestor, and symlink entries are refused.
pub fn extract_zip_archive(zip_path: &Path, destination_dir: &Path) -> Result<()> {
    extract_zip_archive_with_limits(zip_path, destination_dir, ExtractionLimits::default())
}

fn extract_zip_archive_with_limits(
    zip_path: &Path,
    destination_dir: &Path,
    limits: ExtractionLimits,
) -> Result<()> {
    engine::extract_zip_archive_with_platform::<DefaultPlatform>(zip_path, destination_dir, limits)
}
