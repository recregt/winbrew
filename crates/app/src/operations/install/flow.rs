//! Orchestration helpers for the middle and final phases of installation.
//!
//! The outer install entry point uses this module to keep the overall workflow
//! readable: the rollback helpers ensure that partial filesystem state is
//! removed when cancellation or failure interrupts the process, and the engine
//! execution helper keeps the backend call site consistent once the payload has
//! already been downloaded and classified.
//!
//! The functions here deliberately stay focused on execution and cleanup. They
//! do not resolve catalog references or translate errors into user-facing types;
//! that work happens one layer above in [`super::run`].

use anyhow::Result;
use std::path::Path;
use tracing::warn;

use super::state;
use crate::core::fs::{backup_path_for, cleanup_path};
use crate::engines::{EngineKind, PackageEngine};
use crate::models::catalog::CatalogInstaller;
use crate::models::domains::install::EngineInstallReceipt;

/// Remove the temporary root directory used for a single install attempt.
///
/// Cleanup failures are logged as warnings rather than returned to the caller,
/// because install rollback should make a best effort without masking the
/// original failure that triggered cleanup.
pub(crate) fn cleanup_temp_root(temp_root: &Path) {
    if let Err(err) = cleanup_path(temp_root) {
        warn!(
            path = %temp_root.display(),
            error = %err,
            "failed to clean up temporary install root"
        );
    }
}

/// Roll back the database and filesystem state after an install failure.
///
/// The package record is marked as failed and any staged, backup, or final
/// install directories are removed so the next install attempt starts from a
/// clean slate.
pub(crate) fn rollback_failed_install(
    conn: &crate::database::DbConnection,
    name: &str,
    install_dir: &Path,
) {
    let _ = state::mark_failed(conn, name);
    cleanup_install_artifacts(install_dir);
}

/// Roll back the database and filesystem state after a cancelled install.
///
/// Cancellation uses the same cleanup path as a normal failure, but it remains
/// a separate function so the outer workflow can keep cancellation semantics
/// explicit.
pub(crate) fn rollback_cancelled_install(
    conn: &crate::database::DbConnection,
    name: &str,
    install_dir: &Path,
) {
    let _ = state::mark_failed(conn, name);
    cleanup_install_artifacts(install_dir);
}

/// Execute the selected engine against a downloaded payload.
pub(crate) fn execute_engine_install(
    engine: EngineKind,
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    engine.install(installer, download_path, install_dir, package_name)
}

fn cleanup_install_artifacts(install_dir: &Path) {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");
    let backup_dir = backup_path_for(install_dir);

    if let Err(err) = cleanup_path(&stage_dir) {
        warn!(path = %stage_dir.display(), error = %err, "failed to clean up staged install directory");
    }

    if let Err(err) = cleanup_path(&backup_dir) {
        warn!(path = %backup_dir.display(), error = %err, "failed to clean up backup install directory");
    }

    if let Err(err) = cleanup_path(install_dir) {
        warn!(path = %install_dir.display(), error = %err, "failed to clean up install directory");
    }
}
