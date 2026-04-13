//! Persistence layer for WinBrew.
//!
//! `winbrew-storage` owns SQLite access, config persistence, journal replay,
//! and MSI inventory normalization. It deliberately stays close to the runtime
//! database contract so higher layers can use typed helpers instead of direct
//! SQL plumbing.

#![cfg(windows)]

pub use winbrew_core as core;

pub mod database;

pub use database::*;
