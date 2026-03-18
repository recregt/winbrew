use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

use crate::core::{downloader, extractor, paths, shim};
use crate::database::{self, Package, PackageStatus};
use crate::manifest::Manifest;

const PKGS_REPO: &str = "https://raw.githubusercontent.com/yourusername/winbrew-pkgs/main";

pub fn install(name: &str, version: &str, on_progress: impl Fn(u64, u64)) -> Result<()> {
    let conn = database::connect()?;
    database::migrate(&conn)?;

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

    let manifest = fetch_manifest(name, version)?;
    let package_version = manifest.package.version.clone();
    let dependencies = manifest.package.dependencies.clone();
    let source_url = manifest.source.url.clone();
    let checksum = manifest.source.checksum.clone();
    let source_kind = format!("{:?}", manifest.source.kind).to_lowercase();
    let strip_container = manifest.install.strip_container;

    if let Some(pkg) = database::get_package(conn, name)? {
        if pkg.status == PackageStatus::Ok && pkg.version == package_version {
            return Ok(());
        }
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

    let ext = detect_ext(&source_url);
    let cache_file = paths::cache_file(name, &package_version, &ext);
    let install_dir = paths::package_dir(name);

    let normalized_bins = manifest.normalized_bins();
    let shims: Vec<database::Shim> = normalized_bins
        .iter()
        .map(|b| database::Shim {
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
        downloader::download_and_verify(&source_url, &cache_file, &checksum, on_progress)?;

        extractor::extract(&cache_file, &install_dir, strip_container)?;

        for s in &shims {
            let target = install_dir.join(&s.path);
            shim::create(&s.name, &target, s.args.as_deref())?;
        }

        Ok(())
    })();

    if let Err(err) = result {
        cleanup_failed_install(conn, name, &install_dir, &shims);
        return Err(err);
    }

    database::update_status(conn, name, PackageStatus::Ok)?;

    Ok(())
}

fn fetch_manifest(name: &str, version: &str) -> Result<Manifest> {
    let url = format!("{}/{}/{}.toml", PKGS_REPO, name, version);

    let content = reqwest::blocking::get(&url)
        .context("failed to fetch manifest")?
        .error_for_status()
        .context("manifest not found")?
        .text()
        .context("failed to read manifest")?;

    Manifest::from_str(&content)
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
    install_dir: &Path,
    shims: &[database::Shim],
) {
    for shim_entry in shims {
        let _ = shim::remove(&shim_entry.name);
    }

    if install_dir.exists() {
        let _ = std::fs::remove_dir_all(install_dir);
    }

    if let Err(err) = database::update_status(conn, name, PackageStatus::Failed) {
        eprintln!("failed to mark {name} as failed: {err}");
    }
}
