//! Orchestration for the middle and final phases of installation.
//!
//! The outer install entry point uses this module to keep the overall workflow
//! readable: `perform_install` downloads the payload and dispatches it to the
//! selected engine, while the rollback helpers ensure that partial filesystem
//! state is removed when cancellation or failure interrupts the process.
//!
//! The functions here deliberately stay focused on execution and cleanup. They
//! do not resolve catalog references or translate errors into user-facing types;
//! that work happens one layer above in [`super::run`].

use anyhow::Result;
use std::path::Path;
use tracing::warn;

use crate::core::fs::{backup_path_for, cleanup_path};
use crate::core::network::installer_filename;
use crate::engines::{EngineKind, PackageEngine};
use crate::models::domains::install::EngineInstallReceipt;
use crate::models::domains::shared::HashAlgorithm;

use super::download;
use super::state;
use crate::models::catalog::CatalogInstaller;

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

/// Inputs required to execute a single install attempt.
///
/// The outer workflow builds this request once the package, installer, and
/// temporary paths have been resolved. The request keeps the lower-level
/// `perform_install` function focused on execution rather than setup.
pub(crate) struct InstallRequest<'a, FStart, FProgress>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    pub client: &'a crate::core::network::Client,
    pub engine: EngineKind,
    pub installer: &'a CatalogInstaller,
    pub package_name: &'a str,
    pub temp_root: &'a Path,
    pub install_dir: &'a Path,
    pub ignore_checksum_security: bool,
    pub on_start: FStart,
    pub on_progress: FProgress,
}

/// Download the installer and hand it to the selected engine.
///
/// This function performs the core execution phase of installation:
///
/// 1. Derive the temporary download path from the installer URL.
/// 2. Stream the installer to disk while enforcing cancellation checks.
/// 3. Verify the hash strategy selected for the installer.
/// 4. Invoke the engine-specific installation routine.
///
/// The returned tuple contains the engine receipt plus the legacy checksum
/// algorithms that were accepted during download verification, allowing the
/// caller to persist engine-specific cleanup data without reconstructing it.
pub(crate) fn perform_install<FStart, FProgress>(
    request: InstallRequest<'_, FStart, FProgress>,
) -> Result<(EngineInstallReceipt, Vec<HashAlgorithm>)>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let InstallRequest {
        client,
        engine,
        installer,
        package_name,
        temp_root,
        install_dir,
        ignore_checksum_security,
        on_start,
        on_progress,
    } = request;

    let download_path = temp_root.join(installer_filename(&installer.url));
    let legacy_checksum_algorithms = download::download_installer(
        client,
        installer,
        &download_path,
        ignore_checksum_security,
        on_start,
        on_progress,
    )?;

    let engine_receipt = engine.install(installer, &download_path, install_dir, package_name)?;

    Ok((engine_receipt, legacy_checksum_algorithms))
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
