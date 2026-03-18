use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::{
    extractor,
    network::{download_and_verify, fetch_manifest},
    paths, shim,
};
use crate::database;
use crate::models::{Package, PackageStatus, Shim};

pub fn install(name: &str, version: &str, on_progress: impl Fn(u64, u64)) -> Result<()> {
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

    if let Some(pkg) = database::get_package(conn, name)?
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

    let ext = detect_ext(&source_url);
    let cache_file = paths::cache_file(name, &package_version, &ext);
    let install_dir = paths::package_dir_at(&install_root, name);

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
            installed_at: database::now(),
        },
    )?;

    let result = (|| -> Result<()> {
        download_and_verify(conn, &source_url, &cache_file, &checksum, on_progress)?;

        extractor::extract(&cache_file, &install_dir, strip_container)?;

        for s in &shims {
            let target = install_dir.join(&s.path);
            shim::create_at(&install_root, &s.name, &target, s.args.as_deref())?;
        }

        Ok(())
    })();

    if let Err(err) = result {
        cleanup_failed_install(conn, name, &install_root, &install_dir, &shims);
        return Err(err);
    }

    database::update_status(conn, name, PackageStatus::Ok)?;

    Ok(())
}

fn install_root(conn: &rusqlite::Connection) -> Result<PathBuf> {
    Ok(paths::install_root(
        database::config_string(conn, "install_dir")?.as_deref(),
    ))
}

fn detect_ext(url: &str) -> String {
    if url.ends_with(".msi") {
        "msi".to_string()
    } else {
        "zip".to_string()
    }
}

fn parse_dependency(dep: &str) -> (&str, Option<&str>) {
    dep.split_once('@')
        .map(|(name, version)| (name, Some(version)))
        .unwrap_or((dep, None))
}

fn cleanup_failed_install(
    conn: &rusqlite::Connection,
    name: &str,
    install_root: &Path,
    install_dir: &Path,
    shims: &[Shim],
) {
    for shim_entry in shims {
        let _ = shim::remove_at(install_root, &shim_entry.name);
    }

    if install_dir.exists() {
        let _ = std::fs::remove_dir_all(install_dir);
    }

    if let Err(err) = database::update_status(conn, name, PackageStatus::Failed) {
        eprintln!("failed to mark {name} as failed: {err}");
    }
}
