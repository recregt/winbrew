//! Archive extraction facade.

mod extract;
mod kind;
mod platform;

pub use extract::{extract_archive, extract_zip_archive};
pub use kind::ArchiveKind;
