use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::core::paths;
use crate::database;
use crate::manifest::{Manifest, Source};

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub name: String,
    pub package_version: String,
    pub source: Source,
    pub cache_file: PathBuf,
    pub install_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub product_code: Option<String>,
    pub dependencies: Vec<String>,
}

pub fn build_plan(name: &str, manifest: &Manifest) -> Result<InstallPlan> {
    let install_root = install_root();
    let source = manifest
        .selected_source()
        .unwrap_or_else(|| manifest.source.clone());
    let package_version = manifest.package.version.clone();
    let ext = detect_ext(&source.url);
    let cache_file = paths::cache_file(name, &package_version, &ext);
    let install_dir = paths::package_dir_at(&install_root, name);

    Ok(InstallPlan {
        name: name.to_string(),
        package_version,
        source,
        cache_file,
        install_dir: install_dir.clone(),
        backup_dir: install_dir.with_extension("backup"),
        product_code: manifest
            .preferred_installer()
            .and_then(|entry| entry.product_code.clone()),
        dependencies: manifest.package.dependencies.clone(),
    })
}

pub fn install_root() -> PathBuf {
    let config = database::Config::current();
    PathBuf::from(config.paths.root)
}

pub fn detect_ext(url: &str) -> String {
    let url_path = url.split(['?', '#']).next().unwrap_or(url);

    Path::new(url_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("exe")
        .to_string()
}

pub fn source_file_name(url: &str) -> Option<String> {
    let url_path = url.split(['?', '#']).next().unwrap_or(url);

    Path::new(url_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}
