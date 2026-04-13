//! Diagnostic records produced by health and repair scans.

use serde::{Deserialize, Serialize};

/// Diagnostic severity used by health reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosisSeverity {
    /// A failing condition that should be treated as an error.
    Error,
    /// A non-fatal condition that may still require attention.
    Warning,
}

/// A single diagnostic entry emitted by the doctor scan pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosisResult {
    /// Stable machine-readable code for the diagnostic.
    pub error_code: String,
    /// Human-readable description of the problem.
    pub description: String,
    /// Diagnostic severity.
    pub severity: DiagnosisSeverity,
}
