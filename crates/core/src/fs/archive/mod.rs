//! Archive extraction facade.

mod cleanup;
mod context;
mod extract;
mod kind;
mod limits;
mod platform;
mod types;

#[allow(unused_imports)]
pub(crate) use cleanup::ExtractionCleanup;
#[allow(unused_imports)]
pub(crate) use context::ExtractionContext;
#[allow(unused_imports)]
pub(crate) use limits::ExtractionLimits;
#[allow(unused_imports)]
pub(crate) use types::{CachedPath, PathInfo};

pub use extract::{extract_archive, extract_zip_archive};
pub use kind::ArchiveKind;
