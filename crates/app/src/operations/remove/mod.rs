//! End-to-end package removal workflow for `winbrew remove`.
//!
//! Removal is split into two phases so the CLI can reason about impact before
//! it mutates anything:
//!
//! - [`plan_removal`] loads the target package and collects dependent packages.
//! - [`execute_removal`] applies the plan and performs engine-specific cleanup.
//!
//! The high-level [`remove`] helper simply composes those phases for callers
//! that want a one-shot operation. The CLI typically uses the lower-level plan
//! and execution functions separately so it can show a warning, ask for
//! confirmation, and then decide whether to proceed.
//!
//! Dependency checks are conservative by default. If a package is still
//! required by another installed package, removal is blocked unless the caller
//! explicitly opts into `force`. The plan itself is still built so the caller
//! can see exactly which dependents were discovered.

mod execution;
mod plan;

use thiserror::Error;

use crate::models::InstallerType;

pub use crate::models::RemovalPlan;
pub use execution::execute_removal;
pub use plan::{find_dependents, plan_removal};

/// Errors produced by the removal workflow.
///
/// The variants separate policy failures from engine support gaps and from any
/// lower-level runtime error that escapes the removal engine or filesystem
/// cleanup path.
#[derive(Debug, Error)]
pub enum RemovalError {
    /// The package cannot be removed because another installed package still depends on it.
    #[error("cannot remove '{name}' because it is required by: {dependents}")]
    DependentPackagesBlocked { name: String, dependents: String },

    /// The installed package type does not have a supported removal strategy.
    #[error("unsupported package type: {kind}")]
    UnsupportedPackageType { kind: InstallerType },

    /// A lower-level error escaped the removal pipeline.
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

/// Convenience result type for removal operations.
pub type Result<T> = std::result::Result<T, RemovalError>;

/// Plan and execute package removal in one call.
///
/// This helper is intentionally small: it first resolves a removal plan, then
/// hands the plan to the execution layer. It does not perform UI prompting or
/// confirmation logic, so callers that need to warn the user about dependent
/// packages should use [`plan_removal`] directly before deciding whether to
/// call this function.
pub fn remove(name: &str, force: bool) -> Result<()> {
    let plan = plan_removal(name)?;

    execute_removal(&plan, force)
}
