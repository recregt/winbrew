//! Portable install strategy for raw file-based payloads.
//!
//! The routing layer decides when a payload is portable; this module only
//! owns the raw-copy install/remove strategy.

mod install;
mod remove;

pub use install::install;
pub use remove::remove;
