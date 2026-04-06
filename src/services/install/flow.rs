use anyhow::Result;
use std::path::Path;
use tracing::warn;

use crate::core::cancel::CancellationError;
use crate::core::fs::{backup_directory_path, cleanup_path};
use crate::core::hash::HashAlgorithm;
use crate::core::network::installer_filename;
use crate::engines::{EngineKind, PackageEngine};

use super::download;
use super::state;

pub(crate) fn cleanup_temp_root(temp_root: &Path) {
    if let Err(err) = cleanup_path(temp_root) {
        warn!(
            path = %temp_root.display(),
            error = %err,
            "failed to clean up temporary install root"
        );
    }
}

pub(crate) fn rollback_failed_install(
    conn: &rusqlite::Connection,
    name: &str,
    install_dir: &Path,
    temp_root: &Path,
) {
    let _ = state::mark_failed(conn, name);
    cleanup_install_artifacts(install_dir, temp_root);
}

pub(crate) fn rollback_cancelled_install(
    conn: &rusqlite::Connection,
    name: &str,
    install_dir: &Path,
    temp_root: &Path,
) {
    let _ = state::mark_failed(conn, name);
    cleanup_install_artifacts(install_dir, temp_root);
}

pub(crate) fn is_cancelled_error(err: &anyhow::Error) -> bool {
    err.downcast_ref::<CancellationError>().is_some()
}

pub(crate) struct InstallRequest<'a, FStart, FProgress>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    pub client: &'a reqwest::blocking::Client,
    pub engine: EngineKind,
    pub installer: &'a crate::models::CatalogInstaller,
    pub temp_root: &'a Path,
    pub install_dir: &'a Path,
    pub ignore_checksum_security: bool,
    pub on_start: FStart,
    pub on_progress: FProgress,
}

pub(crate) fn perform_install<FStart, FProgress>(
    request: InstallRequest<'_, FStart, FProgress>,
) -> Result<Vec<HashAlgorithm>>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let InstallRequest {
        client,
        engine,
        installer,
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

    engine.install(installer, &download_path, install_dir)?;

    Ok(legacy_checksum_algorithms)
}

fn cleanup_install_artifacts(install_dir: &Path, temp_root: &Path) {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");
    let backup_dir = backup_directory_path(install_dir);

    cleanup_temp_root(temp_root);

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
