//! Workflow layer for WinBrew.
//!
//! `winbrew-app` owns the business-level orchestration for install, update,
//! doctor, repair, and related command flows. It sits between the CLI
//! presentation layer and the lower-level core, database, engines, and models
//! crates, so it can keep execution logic reusable in tests and other callers.

#![cfg(windows)]

pub use winbrew_core as core;
pub use winbrew_database as database;
pub use winbrew_engines as engines;
pub use winbrew_models as models;

mod catalog;
pub mod operations;

pub use operations::{
    AppContext, config, context, doctor, info, install, list, remove, repair, report, search,
    update, version,
};
