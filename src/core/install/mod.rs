pub mod plan;

use anyhow::{Context, Result};

use crate::database;
use crate::models::PackageStatus;

pub use plan::{InstallPlan, build_plan, detect_ext, install_root, source_file_name};

pub fn begin_install(context: &InstallPlan) -> Result<()> {
    crate::core::paths::ensure_dirs()?;
    crate::core::paths::ensure_install_dirs(&install_root())?;

    if context.backup_dir.exists() {
        std::fs::remove_dir_all(&context.backup_dir)
            .context("failed to remove stale backup directory")?;
    }

    if context.install_dir.exists() {
        std::fs::rename(&context.install_dir, &context.backup_dir)
            .context("failed to move current install aside")?;
    }

    std::fs::create_dir_all(&context.install_dir).context("failed to create install directory")?;

    Ok(())
}

pub fn finalize_install(conn: &rusqlite::Connection, context: &InstallPlan) -> Result<()> {
    if context.backup_dir.exists() {
        std::fs::remove_dir_all(&context.backup_dir)
            .context("failed to remove backup directory")?;
    }

    database::update_status(conn, &context.name, PackageStatus::Ok)?;
    Ok(())
}

pub fn fail_install(conn: &rusqlite::Connection, context: &InstallPlan) {
    let _ = std::fs::remove_dir_all(&context.install_dir);

    if context.backup_dir.exists() {
        let _ = std::fs::rename(&context.backup_dir, &context.install_dir);
    }

    let _ = database::update_status(conn, &context.name, PackageStatus::Failed);
}

pub fn insert_installing_package(conn: &rusqlite::Connection, context: &InstallPlan) -> Result<()> {
    use crate::models::Package;

    database::insert_package(
        conn,
        &Package {
            name: context.name.clone(),
            version: context.package_version.clone(),
            kind: context.source.kind.clone(),
            install_dir: context.install_dir.to_string_lossy().to_string(),
            product_code: context.product_code.clone(),
            dependencies: context.dependencies.clone(),
            status: PackageStatus::Installing,
            installed_at: crate::core::time::now(),
        },
    )
}
