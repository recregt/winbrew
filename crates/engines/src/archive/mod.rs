//! Archive-backed install strategy for file-based payloads.
//!
//! The engine decides archive-vs-raw behavior elsewhere; this module only
//! owns the archive-backed install/remove strategy.

mod install;
mod remove;

pub(crate) use install::install;
pub(crate) use remove::remove;
