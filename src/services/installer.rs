use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::{extractor, network::download_and_verify, paths, shim, time};
use crate::database;
use crate::models::{Package, PackageStatus, Shim};
use crate::services::fetch_manifest;
use tracing::{debug, warn};

pub fn install(name: &str, version: &str, on_progress: impl Fn(u64, u64)) -> Result<()> {
    debug!(package = name, version = version, "starting install");

    let conn = database::lock_conn()?;

    let mut visited = HashSet::new();
    install_recursive(name, version, &conn, &mut visited, &on_progress)
}

fn install_recursive(
    name: &str,
    version: &str,
    conn: &rusqlite::Connection,
    visited: &mut HashSet<(String, String)>,
    on_progress: &impl Fn(u64, u64),
) -> Result<()> {
    let visit_key = (name.to_string(), version.to_string());
    if !visited.insert(visit_key) {
        return Ok(());
    }

    let install_root = install_root(conn)?;
    let manifest = fetch_manifest(conn, name, version)?;
    let package_version = manifest.package.version.clone();
    let dependencies = manifest.package.dependencies.clone();
    let source_url = manifest.source.url.clone();
    let checksum = manifest.source.checksum.clone();
    let source_kind = format!("{:?}", manifest.source.kind).to_lowercase();
    let strip_container = manifest.install.strip_container;
    let existing_pkg = database::get_package(conn, name)?;
    let is_update = existing_pkg.is_some();

    if let Some(pkg) = &existing_pkg
        && pkg.status == PackageStatus::Ok
        && pkg.version == package_version
    {
        return Ok(());
    }

    for dep in &dependencies {
        let (dep_name, dep_version) = parse_dependency(dep);
        install_recursive(
            dep_name,
            dep_version.unwrap_or("latest"),
            conn,
            visited,
            on_progress,
        )?;
    }

    paths::ensure_dirs()?;
    paths::ensure_install_dirs(&install_root)?;

    let ext = detect_ext(&source_url, &source_kind);
    let cache_file = paths::cache_file(name, &package_version, &ext);
    let install_dir = paths::package_dir_at(&install_root, name);
    let staging_dir = install_dir.with_extension("staging");
    let backup_dir = install_dir.with_extension("backup");

    let normalized_bins = manifest.normalized_bins();
    let shims: Vec<Shim> = normalized_bins
        .iter()
        .map(|b| Shim {
            name: b.name.clone(),
            path: b.path.clone(),
            args: b.args.clone(),
        })
        .collect();

    database::insert_package(
        conn,
        &Package {
            name: name.to_string(),
            version: package_version.clone(),
            kind: source_kind,
            install_dir: install_dir.to_string_lossy().to_string(),
            shims: shims.clone(),
            dependencies,
            status: PackageStatus::Installing,
            installed_at: time::now(),
        },
    )?;

    let result = (|| -> Result<()> {
        download_and_verify(conn, &source_url, &cache_file, &checksum, on_progress)
            .context("download and verification failed")?;

        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir).context("failed to remove stale staging directory")?;
        }

        extractor::extract(&cache_file, &staging_dir, strip_container)
            .context("extraction failed")?;

        swap_in_staged_install(&staging_dir, &install_dir, &backup_dir)
            .context("failed to finalize installation")?;

        for s in &shims {
            let target = install_dir.join(&s.path);
            shim::create_at(&install_root, &s.name, &target, s.args.as_deref())
                .context("failed to create shim")?;
        }

        Ok(())
    })();

    if let Err(err) = result {
        let cleanup = InstallCleanup {
            conn,
            name,
            install_root: &install_root,
            staging_dir: &staging_dir,
            backup_dir: &backup_dir,
            install_dir: &install_dir,
            shims: &shims,
            is_update,
        };

        cleanup_failed_install(cleanup);
        return Err(err);
    }

    database::update_status(conn, name, PackageStatus::Ok)?;
    debug!(
        package = name,
        version = package_version.as_str(),
        "install completed"
    );

    Ok(())
}

fn install_root(conn: &rusqlite::Connection) -> Result<PathBuf> {
    let _ = conn;

    let config = database::Config::current();
    Ok(PathBuf::from(config.paths.root))
}

fn detect_ext(url: &str, kind: &str) -> String {
    if kind.eq_ignore_ascii_case("msi") {
        "msi".to_string()
    } else {
        let url_path = url.split(['?', '#']).next().unwrap_or(url);

        Path::new(url_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("zip")
            .to_string()
    }
}

fn parse_dependency(dep: &str) -> (&str, Option<&str>) {
    dep.split_once('@')
        .map(|(name, version)| (name, Some(version)))
        .unwrap_or((dep, None))
}

struct InstallCleanup<'a> {
    conn: &'a rusqlite::Connection,
    name: &'a str,
    install_root: &'a Path,
    staging_dir: &'a Path,
    backup_dir: &'a Path,
    install_dir: &'a Path,
    shims: &'a [Shim],
    is_update: bool,
}

fn cleanup_failed_install(ctx: InstallCleanup<'_>) {
    if ctx.staging_dir.exists() {
        let _ = fs::remove_dir_all(ctx.staging_dir);
    }

    if ctx.backup_dir.exists() {
        let _ = fs::remove_dir_all(ctx.backup_dir);
    }

    if !ctx.is_update {
        for shim_entry in ctx.shims {
            let _ = shim::remove_at(ctx.install_root, &shim_entry.name);
        }

        if ctx.install_dir.exists() {
            let _ = fs::remove_dir_all(ctx.install_dir);
        }
    }

    if let Err(err) = database::update_status(ctx.conn, ctx.name, PackageStatus::Failed) {
        warn!("failed to mark {} as failed: {err}", ctx.name);
    }
}

fn swap_in_staged_install(staging_dir: &Path, install_dir: &Path, backup_dir: &Path) -> Result<()> {
    if backup_dir.exists() {
        fs::remove_dir_all(backup_dir).context("failed to remove stale backup directory")?;
    }

    let mut moved_existing_install = false;

    if install_dir.exists() {
        fs::rename(install_dir, backup_dir).context("failed to move current install aside")?;
        moved_existing_install = true;
    }

    match fs::rename(staging_dir, install_dir) {
        Ok(_) => {
            if backup_dir.exists() {
                fs::remove_dir_all(backup_dir).context("failed to remove backup directory")?;
            }

            Ok(())
        }
        Err(err) => {
            if moved_existing_install {
                let _ = fs::rename(backup_dir, install_dir);
            }

            Err(err).context("failed to move staging directory to final installation path")
        }
    }
}
