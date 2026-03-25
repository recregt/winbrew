pub mod plan;
pub mod selection;

use anyhow::{Context, Result};

use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::Duration;

use rusqlite::Connection;

use crate::database;
use crate::models::Package;
use crate::models::PackageStatus;

pub use plan::{InstallPlan, build_plan, detect_ext, install_root, source_file_name};

pub struct InstallTransaction<'a> {
    conn: &'a Connection,
    plan: &'a InstallPlan,
    committed: bool,
}

impl<'a> InstallTransaction<'a> {
    pub fn start(conn: &'a Connection, plan: &'a InstallPlan) -> Result<Self> {
        begin_install(plan)?;

        if let Err(err) = insert_installing_package(conn, plan) {
            fail_install(conn, plan);
            return Err(err);
        }

        Ok(Self {
            conn,
            plan,
            committed: false,
        })
    }

    pub fn commit(mut self) -> Result<()> {
        finalize_install(self.conn, self.plan)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for InstallTransaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            fail_install(self.conn, self.plan);
        }
    }
}

pub fn begin_install(context: &InstallPlan) -> Result<()> {
    crate::core::paths::ensure_dirs()?;
    crate::core::paths::ensure_install_dirs(&install_root())?;

    if context.backup_dir.exists() {
        retry_fs_operation("failed to remove stale backup directory", || {
            fs::remove_dir_all(&context.backup_dir)
        })?;
    }

    if context.install_dir.exists() {
        retry_fs_operation("failed to move current install aside", || {
            fs::rename(&context.install_dir, &context.backup_dir)
        })?;
    }

    retry_fs_operation("failed to create install directory", || {
        fs::create_dir_all(&context.install_dir)
    })?;

    Ok(())
}

pub fn finalize_install(conn: &rusqlite::Connection, context: &InstallPlan) -> Result<()> {
    database::update_status(conn, &context.name, PackageStatus::Ok)?;

    if context.backup_dir.exists() {
        background_delete(&context.backup_dir);
    }

    Ok(())
}

pub fn fail_install(conn: &rusqlite::Connection, context: &InstallPlan) {
    if context.install_dir.exists() {
        background_delete(&context.install_dir);
    }

    if context.backup_dir.exists() {
        let _ = retry_fs_operation("failed to restore backup install directory", || {
            fs::rename(&context.backup_dir, &context.install_dir)
        });
    }

    let _ = database::update_status(conn, &context.name, PackageStatus::Failed);
}

pub fn insert_installing_package(conn: &rusqlite::Connection, context: &InstallPlan) -> Result<()> {
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

fn retry_fs_operation<T, F>(description: &str, mut operation: F) -> Result<T>
where
    F: FnMut() -> io::Result<T>,
{
    let mut delay = Duration::from_millis(50);
    let attempts = 4;

    for attempt in 0..attempts {
        match operation() {
            Ok(value) => return Ok(value),
            Err(err) if attempt + 1 < attempts && is_retryable_fs_error(&err) => {
                thread::sleep(delay);
                delay = delay.saturating_add(delay);
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("{description} (failed after {attempts} attempts)"));
            }
        }
    }

    unreachable!("retry loop must return on success or failure")
}

fn is_retryable_fs_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::WouldBlock
    ) || matches!(err.raw_os_error(), Some(5 | 32 | 145 | 183))
}

fn background_delete(path: &Path) {
    if !path.exists() {
        return;
    }

    let Some(folder_name) = path.file_name().and_then(|name| name.to_str()) else {
        let _ = fs::remove_dir_all(path);
        return;
    };

    let temp_name = format!("{folder_name}.tmp_{}", crate::core::time::now_ms());
    let temp_path = path.with_file_name(temp_name);

    let target = match fs::rename(path, &temp_path) {
        Ok(()) => temp_path,
        Err(_) => path.to_path_buf(),
    };

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        let _ = fs::remove_dir_all(target);
    });
}
