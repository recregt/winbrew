//! Archive extraction facade.

mod cleanup;
mod context;
mod extract;
mod kind;
mod limits;
mod platform;
mod types;

pub use extract::{extract_archive, extract_zip_archive};
pub use kind::ArchiveKind;
