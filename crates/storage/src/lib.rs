//! Persistence layer for WinBrew.
//!
//! `winbrew-storage` owns SQLite access, config persistence, journal replay,
//! and MSI inventory normalization. It stays close to the runtime database
//! contract so higher layers can use typed helpers instead of direct SQL
//! plumbing.
//!
//! The database module keeps its pool registry keyed by resolved paths. That
//! makes the current process-local root model explicit while still keeping the
//! storage boundary centralized for the app and CLI layers.

#![cfg(windows)]

pub use winbrew_core as core;

pub mod database;

pub use database::*;
