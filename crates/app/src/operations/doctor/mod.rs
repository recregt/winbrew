//! # Doctor Operations
//!
//! Health checks and package integrity verification for the `doctor` command.
//!
//! The public API is intentionally small: callers use [`health_report`] to build a
//! [`crate::models::HealthReport`], while the package scanning pipeline stays internal.

mod report;
mod scan;

pub use report::health_report;
