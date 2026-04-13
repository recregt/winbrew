//! Diagnostics, info, and recovery reports.
//!
//! This family owns the user-facing reporting surface produced by the doctor
//! workflow and related health checks. Keep diagnostic severity, recovery
//! issue mapping, and final report assembly logic here so the app layer can
//! remain focused on orchestration and rendering.

pub mod diagnostics;
pub mod info;
pub mod report;

pub use diagnostics::{DiagnosisResult, DiagnosisSeverity};
pub use info::InfoReport;
pub use report::{
    HealthReport, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind, ReportSection,
    RuntimeReport,
};
