use anyhow::{Result, anyhow, bail};
use std::path::{Component, Path, PathBuf};

use crate::core::install::selection;
use crate::core::paths;
use crate::database;
use crate::manifest::{InstallerEntry, Manifest, Source};

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
    validate_package_name(name)?;

    let install_root = install_root();
    let selected_installer = selection::select_installer(&manifest.installers);
    let source = selected_installer
        .map(InstallerEntry::to_source)
        .or_else(|| manifest.source.clone())
        .ok_or_else(|| anyhow!("manifest must define a source or installer"))?;
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
        backup_dir: backup_dir_for(&install_dir),
        product_code: selected_installer.and_then(|entry| entry.product_code.clone()),
        dependencies: manifest.package.dependencies.clone(),
    })
}

pub fn backup_dir_for(install_dir: &Path) -> PathBuf {
    install_dir.with_extension("backup")
}

pub fn install_root() -> PathBuf {
    let (root, _) = database::get_effective_value("paths.root").unwrap_or_else(|_| {
        (
            database::Config::current().paths.root,
            database::ConfigSource::File,
        )
    });

    PathBuf::from(root)
}

pub fn detect_ext(url: &str) -> String {
    let url_path = url.split(['?', '#']).next().unwrap_or(url);

    Path::new(url_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("exe")
        .to_ascii_lowercase()
}

pub fn source_file_name(url: &str) -> Option<String> {
    let url_path = url.split(['?', '#']).next().unwrap_or(url);

    Path::new(url_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

fn validate_package_name(name: &str) -> Result<()> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        bail!("invalid package name: empty value");
    }

    if trimmed
        .chars()
        .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
    {
        bail!("invalid package name: forbidden Windows filename characters detected");
    }

    if !Path::new(trimmed)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("invalid package name: traversal characters detected");
    }

    Ok(())
}
