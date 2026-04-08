use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub packages: PathBuf,
    pub data: PathBuf,
    pub logs: PathBuf,
    pub cache: PathBuf,
    pub db: PathBuf,
    pub catalog_db: PathBuf,
    pub config: PathBuf,
    pub log: PathBuf,
}

pub fn config_file_at(root: &Path) -> PathBuf {
    root.join("data").join("winbrew.toml")
}

pub fn packages_dir_at(root: &Path) -> PathBuf {
    root.join("packages")
}

pub fn data_dir_at(root: &Path) -> PathBuf {
    root.join("data")
}

pub fn db_dir_at(root: &Path) -> PathBuf {
    data_dir_at(root).join("db")
}

pub fn db_path_at(root: &Path) -> PathBuf {
    db_dir_at(root).join("winbrew.db")
}

pub fn catalog_db_at(root: &Path) -> PathBuf {
    db_dir_at(root).join("catalog.db")
}

pub fn log_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("logs")
}

pub fn log_file_at(root: &Path) -> PathBuf {
    log_dir_at(root).join("winbrew.log")
}

pub fn cache_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("cache")
}

pub fn cache_file_at(root: &Path, name: &str, version: &str, ext: &str) -> PathBuf {
    cache_dir_at(root).join(cache_filename(name, version, ext))
}

fn cache_filename(name: &str, version: &str, ext: &str) -> String {
    let mut filename = String::with_capacity(name.len() + version.len() + ext.len() + 2);
    filename.push_str(name);
    filename.push('-');
    filename.push_str(version);
    filename.push('.');
    filename.push_str(ext);
    filename
}

pub fn ensure_dirs_at(root: &Path) -> std::io::Result<()> {
    for dir in [
        packages_dir_at(root),
        data_dir_at(root),
        db_dir_at(root),
        log_dir_at(root),
        cache_dir_at(root),
    ] {
        std::fs::create_dir_all(dir)?;
    }

    Ok(())
}

pub fn ensure_install_dirs_at(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(packages_dir_at(root))
}

pub fn resolve_template(root: &Path, template: &str) -> PathBuf {
    let root_text = root.to_string_lossy();

    if template.contains("${root}") {
        PathBuf::from(template.replace("${root}", &root_text))
    } else {
        PathBuf::from(template)
    }
}

pub fn resolved_paths(
    root: &Path,
    packages: &str,
    data: &str,
    logs: &str,
    cache: &str,
) -> ResolvedPaths {
    let root = PathBuf::from(root);
    let data = resolve_template(&root, data);
    let logs = resolve_template(&root, logs);
    let db = data.join("db");

    ResolvedPaths {
        packages: resolve_template(&root, packages),
        cache: resolve_template(&root, cache),
        db: db.join("winbrew.db"),
        catalog_db: db.join("catalog.db"),
        config: data.join("winbrew.toml"),
        log: logs.join("winbrew.log"),
        root,
        data,
        logs,
    }
}

pub fn install_root_from_package_dir(install_dir: &Path) -> PathBuf {
    install_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_default()
}
