//! Health reporting for installed packages and install roots.
//!
//! The doctor workflow provides a single public entry point, [`health_report`],
//! that gathers database and filesystem diagnostics into a
//! [`crate::models::HealthReport`]. The implementation stays split across
//! internal helper modules so the report assembly code does not need to know the
//! details of package scanning or diagnostic formatting.
//!
//! The pipeline is intentionally narrow:
//!
//! - `report` assembles the final report structure and summary counts.
//! - `scan` is split into `package`, `msi`, `journal`, and `orphan` helpers so recovery policy logic can evolve independently.
//!
//! CLI code owns any interactive presentation around the report, including the
//! spinner and terminal formatting. The app layer only returns structured data.

mod report;
mod scan;

pub use report::health_report;
