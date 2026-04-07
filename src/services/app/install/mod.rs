pub mod download;
pub mod flow;
pub mod state;
pub mod types;

use std::fs;

use crate::AppContext;
use crate::core::cancel;
use crate::database;
use crate::engines::{self, EngineKind};
use crate::models::{CatalogPackage, PackageRef};
use crate::services::shared::catalog;
use crate::services::shared::temp_workspace;

pub use crate::models::install::InstallFailureClass;
pub use crate::models::install::{InstallOutcome, InstallResult};
pub use types::InstallError;
pub type Result<T> = types::Result<T>;

pub fn run<FChoose, FStart, FProgress>(
    ctx: &AppContext,
    package_ref: PackageRef,
    ignore_checksum_security: bool,
    mut choose_package: FChoose,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<InstallOutcome>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> anyhow::Result<usize>,
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let catalog_conn = database::get_catalog_conn()?;
    let package =
        catalog::resolve_catalog_package_ref(&catalog_conn, &package_ref, &mut choose_package)?;
    let installer =
        catalog::select_installer(&database::get_installers(&catalog_conn, &package.id)?)?;
    let engine = engines::get_engine(&installer)?;
    let package_version = package.version.to_string();

    let install_dir = ctx.paths.packages.join(&package.name);
    let temp_root = temp_workspace::build_temp_root(&package.name, &package_version);

    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&temp_root)?;

    let _temp_root_guard = TempRootGuard::new(temp_root.clone());

    let conn = database::get_conn()?;
    state::prepare_install_target(&conn, &package.name, &install_dir)?;
    state::mark_installing(
        &conn,
        package.name.clone(),
        package_version.clone(),
        installer.kind.to_string(),
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
        on_start,
        on_progress,
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

    if cancel::is_cancelled() {
        flow::rollback_cancelled_install(&conn, &package.name, &install_dir);
        return Err(cancel::CancellationError.into());
    }

    let install_result = InstallResult {
        name: package.name,
        version: package_version,
        install_dir: install_dir.to_string_lossy().to_string(),
    };

    let msix_package_full_name = if engine == EngineKind::Msix {
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
    path: std::path::PathBuf,
}

impl TempRootGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempRootGuard {
    fn drop(&mut self) {
        flow::cleanup_temp_root(&self.path);
    }
}
