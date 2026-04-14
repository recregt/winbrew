//! Engine-specific package removal and filesystem cleanup.
//!
//! The execution phase consumes a precomputed [`RemovalPlan`] and mutates the
//! database and filesystem state accordingly. The exact removal strategy
//! depends on the package engine:
//!
//! - MSIX, MSI, and native executable packages are removed through the engine
//!   first and then cleaned from disk.
//! - Zip and portable packages are staged into a trash directory before the
//!   database row is deleted so the install tree can be restored if metadata
//!   removal fails.
//!
//! The functions here favor best-effort cleanup. Filesystem failures after the
//! removal path has already made progress are logged when practical so the main
//! removal outcome stays focused on whether the package was successfully removed.

use anyhow::Context;
use tracing::{debug, warn};

use std::path::PathBuf;

use crate::core::fs::cleanup_path;
use crate::database;
use crate::engines::{EngineKind, PackageEngine};

use super::{RemovalError, RemovalPlan, Result};
use crate::models::domains::installed::InstalledPackage;

/// Execute package removal using a fresh database connection.
///
/// This is the public execution entry point. It only acquires the database
/// connection and then delegates the actual work to the shared removal engine.
pub fn execute_removal(plan: &RemovalPlan, force: bool) -> Result<()> {
    let conn = database::get_conn()?;

    execute_removal_with_conn(plan, force, &conn)
}

/// Execute a removal plan with a caller-provided database connection.
///
/// This function enforces the removal policy, selects the correct engine kind,
/// and applies the engine-specific cleanup path. When `force` is false, the
/// presence of dependent packages blocks removal before any mutation happens.
fn execute_removal_with_conn(
    plan: &RemovalPlan,
    force: bool,
    conn: &database::DbConnection,
) -> Result<()> {
    debug!(
        package = plan.package.name.as_str(),
        force, "starting remove"
    );

    if !force && !plan.dependents.is_empty() {
        // Remove a package directory, database row, and any leftover staging artifacts.
        // The helper is intentionally engine-agnostic. It handles the shared cleanup
        // patterns used by both removal strategies and leaves the engine-specific work
        // to the caller.
        return Err(RemovalError::DependentPackagesBlocked {
            name: plan.package.name.clone(),
            dependents: plan.dependents.join(", "),
        });
    }

    let install_dir = PathBuf::from(&plan.package.install_dir);
    let engine_kind = plan.package.engine_kind;

    match engine_kind {
        EngineKind::Msix | EngineKind::Msi | EngineKind::NativeExe => {
            engine_kind.remove(&plan.package)?;

            if install_dir.exists()
                && let Err(err) = cleanup_path(&install_dir)
            {
                warn!(
                    "failed to remove package directory for {}: {err}",
                    plan.package.name
                );
            }

            database::delete_package(conn, &plan.package.name)?;
        }
        EngineKind::Zip | EngineKind::Portable => {
            if install_dir.exists() {
                let trash_dir = install_dir.with_extension("trash");

                cleanup_path(&trash_dir).context("failed to clean up old trash directory")?;

                std::fs::rename(&install_dir, &trash_dir)
                    .context("failed to stage package for removal")?;

                let trash_package = InstalledPackage {
                    install_dir: trash_dir.to_string_lossy().into_owned(),
                    ..plan.package.clone()
                };

                if let Err(err) = database::delete_package(conn, &plan.package.name) {
                    let _ = std::fs::rename(&trash_dir, &install_dir);
                    return Err(RemovalError::Unexpected(err));
                }

                if let Err(err) = engine_kind.remove(&trash_package) {
                    warn!(
                        "failed to completely remove trash for {}: {err}",
                        plan.package.name
                    );
                }
            } else {
                database::delete_package(conn, &plan.package.name)?;
            }
        }
    }

    debug!(
        package = plan.package.name.as_str(),
        force, "remove completed"
    );

    Ok(())
}
