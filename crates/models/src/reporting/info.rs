//! Runtime information report exposed by the info command.

use super::report::RuntimeReport;

/// High-level runtime information for display in the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoReport {
    /// Winbrew version string.
    pub version: String,
    /// Structured runtime sections for rendering.
    pub runtime: RuntimeReport,
}
