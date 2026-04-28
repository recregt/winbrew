use anyhow::{Context, Result};

use crate::AppContext;
use crate::catalog;
use crate::database;
use crate::engines::{self, EngineKind};
use crate::models::catalog::{CatalogInstaller, CatalogPackage};
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::package::{PackageId, PackageRef};
use crate::operations::install::{self, InstallObserver};
use crate::operations::remove;

#[derive(Debug, Clone)]
pub struct ResolvedFileRestoreTarget {
    pub package: CatalogPackage,
    pub installer: CatalogInstaller,
    pub engine: EngineKind,
    pub installed_package: InstalledPackage,
}

#[derive(Debug, Clone)]
pub struct FileRestoreReinstallTarget {
    pub catalog_package: CatalogPackage,
    pub installed_version: String,
}

#[derive(Debug, Clone)]
pub enum FileRestoreResolution {
    Restore(Box<ResolvedFileRestoreTarget>),
    Reinstall(Box<FileRestoreReinstallTarget>),
}

/// Resolve a catalog package for repair using the same matching policy as install.
pub fn resolve_repair_catalog_package<FChoose>(
    package_name: &str,
    choose_package: FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let catalog_conn = crate::database::get_catalog_conn()?;
    resolve_repair_catalog_package_with_conn(&catalog_conn, package_name, choose_package)
}

fn resolve_repair_catalog_package_with_conn<FChoose>(
    catalog_conn: &crate::database::DbConnection,
    package_name: &str,
    choose_package: FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let package_ref = PackageRef::parse(package_name)
        .with_context(|| format!("failed to parse package reference '{package_name}'"))?;

    catalog::resolve_catalog_package_ref(catalog_conn, &package_ref, choose_package)
}

/// Resolve a file-restore target and decide whether reinstall is required.
pub fn resolve_file_restore_target<FChoose>(
    package_name: &str,
    choose_package: FChoose,
) -> Result<FileRestoreResolution>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let catalog_conn = crate::database::get_catalog_conn()?;
    let conn = database::get_conn()?;
    let package =
        resolve_repair_catalog_package_with_conn(&catalog_conn, package_name, choose_package)?;
    let installed_package = database::get_package(&conn, package_name)?
        .with_context(|| format!("package '{package_name}' is not installed"))?;

    if installed_package.version != package.version.to_string() {
        return Ok(FileRestoreResolution::Reinstall(Box::new(
            FileRestoreReinstallTarget {
                catalog_package: package,
                installed_version: installed_package.version,
            },
        )));
    }

    let installers = crate::database::get_installers(&catalog_conn, &package.id)?;
    let selection_context = crate::catalog::SelectionContext::new(
        crate::windows::host::host_profile(),
        crate::windows::host::is_elevated(),
    );
    let installer = install::types::select_installer(&installers, selection_context)?;
    let engine = engines::resolve_engine_for_installer(&installer)?;

    if engine_requires_reinstall_only(engine) {
        return Ok(FileRestoreResolution::Reinstall(Box::new(
            FileRestoreReinstallTarget {
                catalog_package: package,
                installed_version: installed_package.version,
            },
        )));
    }

    Ok(FileRestoreResolution::Restore(Box::new(
        ResolvedFileRestoreTarget {
            package,
            installer,
            engine,
            installed_package,
        },
    )))
}

pub(crate) fn engine_requires_reinstall_only(engine: EngineKind) -> bool {
    matches!(engine, EngineKind::Font)
}

/// Reinstall a package using the exact catalog package that was already chosen.
pub fn reinstall_package<O: InstallObserver>(
    ctx: &AppContext,
    catalog_package: &CatalogPackage,
    observer: &mut O,
) -> Result<install::InstallOutcome> {
    let conn = database::get_conn()?;

    if database::get_package(&conn, &catalog_package.name)?.is_some() {
        remove::remove(&catalog_package.name, true).with_context(|| {
            format!(
                "failed to remove package before repair: {}",
                catalog_package.name
            )
        })?;
    }

    let package_ref = PackageRef::ById(
        PackageId::parse(catalog_package.id.as_str())
            .with_context(|| format!("failed to parse catalog id '{}'", catalog_package.id))?,
    );

    install::run(ctx, package_ref, false, observer)
        .with_context(|| format!("failed to reinstall package '{}'", catalog_package.name))
}

#[cfg(test)]
mod tests {
    use super::engine_requires_reinstall_only;
    use crate::engines::EngineKind;

    #[test]
    fn engine_requires_reinstall_only_returns_true_for_fonts() {
        assert!(engine_requires_reinstall_only(EngineKind::Font));
        assert!(!engine_requires_reinstall_only(EngineKind::NativeExe));
        assert!(!engine_requires_reinstall_only(EngineKind::Portable));
    }
}
