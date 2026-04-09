//! ZIP archive extraction facade.

mod extract;
#[cfg(not(windows))]
mod portable;
#[cfg(windows)]
mod windows;

pub use extract::extract_zip_archive;
pub(super) use extract::{CachedPath, ExtractionContext, PathInfo};
