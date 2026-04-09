#![cfg(windows)]

use std::path::PathBuf;

pub use winbrew_catalog as catalog;
pub use winbrew_core as core;
pub use winbrew_engines as engines;
pub use winbrew_models::{
    CatalogPackage, InstallFailureClass, InstallOutcome, InstallResult, PackageRef,
};
pub use winbrew_runtime as runtime;
pub use winbrew_storage as storage;

pub mod download;
pub mod flow;
pub mod state;
mod temp_workspace;
pub mod types;

pub use types::InstallError;
pub type Result<T> = types::Result<T>;

pub trait InstallObserver {
    fn choose_package(&mut self, query: &str, matches: &[CatalogPackage]) -> anyhow::Result<usize>;
    fn on_start(&mut self, total_bytes: Option<u64>);
    fn on_progress(&mut self, downloaded_bytes: u64);
}

pub fn run<O: InstallObserver>(
    paths: &core::ResolvedPaths,
    package_ref: PackageRef,
    ignore_checksum_security: bool,
    observer: &mut O,
) -> Result<InstallOutcome> {
    use std::cell::RefCell;
    use std::fs;

    let observer = RefCell::new(observer);
    let catalog_conn = storage::get_catalog_conn()?;
    let package = catalog::resolve_catalog_package_ref(
        &catalog_conn,
        &package_ref,
        &mut |query, matches| observer.borrow_mut().choose_package(query, matches),
    )?;
    let installer =
        catalog::select_installer(&storage::get_installers(&catalog_conn, &package.id)?)?;
    let engine = engines::get_engine(&installer)?;
    let package_version = package.version.to_string();

    let install_dir = paths.packages.join(&package.name);
    let temp_root = temp_workspace::build_temp_root(&package.name, &package_version);

    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&temp_root)?;

    let _temp_root_guard = TempRootGuard::new(temp_root.clone());

    let conn = storage::get_conn()?;
    state::prepare_install_target(&conn, &package.name, &install_dir)?;
    state::mark_installing(
        &conn,
        package.name.clone(),
        package_version.clone(),
        installer.kind,
        &install_dir,
    )?;

    let client = download::build_client()?;

    let legacy_checksum_algorithms = match flow::perform_install(flow::InstallRequest {
        client: &client,
        engine,
        installer: &installer,
        temp_root: &temp_root,
        install_dir: &install_dir,
        ignore_checksum_security,
        on_start: |total_bytes| observer.borrow_mut().on_start(total_bytes),
        on_progress: |downloaded_bytes| observer.borrow_mut().on_progress(downloaded_bytes),
    }) {
        Ok(legacy_checksum_algorithms) => legacy_checksum_algorithms,
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

    if runtime::is_cancelled() {
        flow::rollback_cancelled_install(&conn, &package.name, &install_dir);
        return Err(runtime::CancellationError.into());
    }

    let install_result = InstallResult {
        name: package.name,
        version: package_version,
        install_dir: install_dir.to_string_lossy().to_string(),
    };

    let msix_package_full_name = if engine == engines::EngineKind::Msix {
        match engines::msix::installed_package_full_name(&install_result.name) {
            Ok(full_name) => Some(full_name),
            Err(err) => {
                flow::rollback_failed_install(&conn, &install_result.name, &install_dir);
                return Err(err.into());
            }
        }
    } else {
        None
    };

    if let Err(err) = state::mark_ok(
        &conn,
        &install_result.name,
        msix_package_full_name.as_deref(),
    ) {
        let _ = state::mark_failed(&conn, &install_result.name);
        return Err(err.into());
    }

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
