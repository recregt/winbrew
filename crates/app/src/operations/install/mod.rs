//! End-to-end installation workflow for `winbrew install`.
//!
//! This module owns the full package installation pipeline once a package
//! reference has been handed off by the CLI layer. The workflow is intentionally
//! split into small submodules so each phase has a clear responsibility:
//!
//! - [`state`] manages database transitions and rejects conflicting installs.
//! - [`download`] builds the network client and downloads the installer payload.
//! - [`flow`] coordinates the download, engine execution, and rollback paths.
//! - [`types`] normalizes lower-level failures into user-facing install errors.
//!
//! The public entry point is [`run`]. It resolves the package reference against
//! the catalog, selects the installer, creates a temporary workspace, streams
//! download progress through [`InstallObserver`], and either commits the final
//! install record or rolls back all partial state on failure.
//!
//! Checksum handling is strict by default. Legacy algorithms such as MD5 and
//! SHA-1 are rejected unless the caller explicitly opts into
//! `ignore_checksum_security`. When that flag is enabled, the accepted legacy
//! algorithms are still returned in [`InstallOutcome`] so the caller can report
//! what was tolerated during verification.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

use crate::catalog;
use crate::core::paths::{ensure_install_dirs_at, install_root_from_package_dir};
use crate::core::temp_workspace;
use crate::engines;
use crate::storage;

pub use crate::core::cancel;
pub use crate::models::{
    CatalogPackage, InstallFailureClass, InstallOutcome, InstallResult, PackageRef,
};
pub use types::InstallError;
pub type Result<T> = types::Result<T>;

pub mod download;
pub mod flow;
pub mod state;
pub mod types;

/// Interactive hooks used by the installation pipeline.
///
/// The install flow uses this trait for the two pieces of user interaction it
/// needs to support: choosing between multiple catalog matches and reporting
/// download progress. Implementations should stay responsive because these
/// callbacks are invoked while the install is actively running.
pub trait InstallObserver {
    /// Choose one package from the catalog matches returned for a reference.
    ///
    /// The callback receives the original query string and the resolved match
    /// set. Return the index of the package to install from the provided slice.
    fn choose_package(&mut self, query: &str, matches: &[CatalogPackage]) -> anyhow::Result<usize>;

    /// Signal that installer download is about to start.
    ///
    /// `total_bytes` is `Some` when the server provided a content length and
    /// `None` when the size is unknown ahead of time.
    fn on_start(&mut self, total_bytes: Option<u64>);

    /// Report cumulative installer download progress in bytes.
    fn on_progress(&mut self, downloaded_bytes: u64);
}

/// Execute the full install workflow for a resolved package reference.
///
/// The function performs the following high-level steps:
///
/// 1. Resolve the package reference against the catalog.
/// 2. Select an installer and build the engine-specific execution context.
/// 3. Prepare the install target by rejecting conflicting database state and
///    clearing stale failed records.
/// 4. Mark the package as installing and create a temporary workspace rooted in
///    the package/version pair.
/// 5. Download and verify the installer while forwarding progress callbacks.
/// 6. Hand the verified payload to the selected engine.
/// 7. Roll back partial state on cancellation or failure, or mark the install
///    as successful when the engine completes.
///
/// On success, the returned [`InstallOutcome`] contains the final install
/// record plus any legacy checksum algorithms that were tolerated during
/// verification. On error, the function maps the underlying failure into
/// [`InstallError`] and makes a best effort to clean up database and filesystem
/// artifacts before returning.
pub fn run<O: InstallObserver>(
    ctx: &crate::AppContext,
    package_ref: PackageRef,
    ignore_checksum_security: bool,
    observer: &mut O,
) -> Result<InstallOutcome> {
    let observer = RefCell::new(observer);
    let catalog_conn = storage::get_catalog_conn()?;
    let package =
        catalog::resolve_catalog_package_ref(&catalog_conn, &package_ref, |query, matches| {
            observer.borrow_mut().choose_package(query, matches)
        })?;
    let installer = types::select_installer(&storage::get_installers(&catalog_conn, &package.id)?)?;
    let engine = engines::resolve_engine_for_installer(&installer)?;
    let package_version = package.version.to_string();

    let install_dir = ctx.paths.package_install_dir(&package.name);
    let temp_root = temp_workspace::build_temp_root(&package.name, &package_version);
    let install_root = install_root_from_package_dir(&install_dir);

    ensure_install_dirs_at(&install_root)?;
    fs::create_dir_all(&temp_root)?;

    let _temp_root_guard = TempRootGuard::new(temp_root.clone());

    let mut conn = storage::get_conn()?;
    state::prepare_install_target(&conn, &package.name, &install_dir)?;
    state::mark_installing(
        &conn,
        package.name.clone(),
        package_version.clone(),
        installer.kind,
        engine,
        &install_dir,
    )?;

    let client = download::build_client()?;

    let (engine_receipt, legacy_checksum_algorithms) =
        match flow::perform_install(flow::InstallRequest {
            client: &client,
            engine,
            installer: &installer,
            package_name: &package.name,
            temp_root: &temp_root,
            install_dir: &install_dir,
            ignore_checksum_security,
            on_start: |total_bytes| observer.borrow_mut().on_start(total_bytes),
            on_progress: |downloaded_bytes| observer.borrow_mut().on_progress(downloaded_bytes),
        }) {
            Ok(result) => result,
            Err(err) => {
                let install_error: InstallError = err.into();

                match install_error.failure_class() {
                    InstallFailureClass::Cancelled => {
                        flow::rollback_cancelled_install(&conn, &package.name, &install_dir);
                    }
                    _ => {
                        flow::rollback_failed_install(&conn, &package.name, &install_dir);
                    }
                }

                return Err(install_error);
            }
        };

    if cancel::is_cancelled() {
        flow::rollback_cancelled_install(&conn, &package.name, &install_dir);
        return Err(cancel::CancellationError.into());
    }

    if let Err(err) = storage::commit_install(&mut conn, &package.name, &engine_receipt) {
        let _ = state::mark_failed(&conn, &package.name);
        return Err(err.into());
    }

    let install_result = InstallResult {
        name: package.name,
        version: package_version,
        install_dir: engine_receipt.install_dir.clone(),
    };

    Ok(InstallOutcome {
        result: install_result,
        legacy_checksum_algorithms,
    })
}

struct TempRootGuard {
    path: PathBuf,
}

impl TempRootGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempRootGuard {
    fn drop(&mut self) {
        flow::cleanup_temp_root(&self.path);
    }
}
