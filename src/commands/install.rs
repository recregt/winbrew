use anyhow::Result;
use std::fs;

use crate::core::paths;
use crate::database;
use crate::services::install::{
    catalog, download, staging, state, types::InstallResult, workspace,
};
use crate::ui::Ui;

pub fn run(query: &[String]) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Install Package");

    let query_text = query.join(" ");
    ui.info(format!("Resolving {query_text}..."));

    let progress = ui.progress_bar();

    let result = install_package(
        &query_text,
        |total_bytes| {
            if let Some(total_bytes) = total_bytes {
                progress.set_length(total_bytes);
            }
            progress.set_message("Downloading installer");
        },
        |downloaded_bytes| {
            progress.inc(downloaded_bytes);
        },
    );

    progress.finish_and_clear();

    let result = result?;
    ui.success(format!(
        "Installed {} {} into {}.",
        result.name, result.version, result.install_dir
    ));

    Ok(())
}

fn install_package<FStart, FProgress>(
    query: &str,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<InstallResult>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let query = query.trim();
    if query.is_empty() {
        anyhow::bail!("package query cannot be empty");
    }

    let catalog_conn = database::get_catalog_conn()?;
    let package = catalog::resolve_catalog_package(&catalog_conn, query)?;
    let installer =
        catalog::select_installer(&database::get_installers(&catalog_conn, &package.id)?)?;

    let package_name = package.name.clone();
    let package_version = package.version.clone();
    let install_dir = paths::package_dir(&package_name);
    let temp_root = workspace::build_temp_root(&package_name, &package_version);
    let stage_dir = install_dir.with_extension("staging");

    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&temp_root)?;

    let conn = database::get_conn()?;
    state::prepare_install_target(&conn, &package_name, &install_dir)?;
    state::mark_installing(
        &conn,
        &package_name,
        &package_version,
        &installer.kind,
        &install_dir,
    )?;

    let result = (|| -> Result<InstallResult> {
        let download_path = temp_root.join(download::installer_filename(&installer.url));
        download::download_installer(&installer, &download_path, on_start, on_progress)?;
        staging::stage_installer(&installer, &download_path, &stage_dir, &package_name)?;
        staging::replace_directory(&stage_dir, &install_dir)?;
        state::mark_ok(&conn, &package_name)?;

        Ok(InstallResult {
            name: package_name.clone(),
            version: package_version.clone(),
            install_dir: install_dir.to_string_lossy().to_string(),
        })
    })();

    match result {
        Ok(result) => {
            let _ = staging::cleanup_path(&temp_root);
            Ok(result)
        }
        Err(err) => {
            let _ = state::mark_failed(&conn, &package_name);
            let _ = staging::cleanup_path(&stage_dir);
            let _ = staging::cleanup_path(&temp_root);
            Err(err)
        }
    }
}
