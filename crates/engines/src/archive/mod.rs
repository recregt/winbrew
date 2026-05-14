//! Archive-backed install strategy for file-based payloads.
//!
//! The engine decides archive-vs-raw behavior elsewhere; this module only
//! owns the archive-backed install/remove strategy.

mod install;
mod remove;

pub use install::install;
pub use remove::remove;
