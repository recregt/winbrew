use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub packages: PathBuf,
    pub data: PathBuf,
    pub logs: PathBuf,
    pub cache: PathBuf,
    pub db: PathBuf,
    pub config: PathBuf,
    pub log: PathBuf,
}

pub fn config_file_at(root: &Path) -> PathBuf {
    root.join("data").join("winbrew.toml")
}

pub fn base_dir() -> PathBuf {
    crate::database::Config::current().resolved_paths().root
}

pub fn packages_dir() -> PathBuf {
    base_dir().join("packages")
}

pub fn packages_dir_at(root: &Path) -> PathBuf {
    root.join("packages")
}

pub fn install_root(value: Option<&str>) -> PathBuf {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(base_dir)
}

pub fn package_dir(name: &str) -> PathBuf {
    base_dir().join("packages").join(name)
}

pub fn package_dir_at(root: &Path, name: &str) -> PathBuf {
    packages_dir_at(root).join(name)
}

pub fn data_dir() -> PathBuf {
    base_dir().join("data")
}

pub fn db_dir() -> PathBuf {
    data_dir().join("db")
}

pub fn db_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("db")
}

pub fn db_path() -> PathBuf {
    db_dir().join("winbrew.db")
}

pub fn config_file() -> PathBuf {
    config_file_at(&base_dir())
}

pub fn log_dir() -> PathBuf {
    base_dir().join("data").join("logs")
}

pub fn log_file() -> PathBuf {
    log_dir().join("winbrew.log")
}

pub fn cache_dir() -> PathBuf {
    base_dir().join("data").join("cache")
}

pub fn cache_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("cache")
}

pub fn cache_file(name: &str, version: &str, ext: &str) -> PathBuf {
    cache_dir().join(format!("{}-{}.{}", name, version, ext))
}

pub fn cache_file_at(root: &Path, name: &str, version: &str, ext: &str) -> PathBuf {
    cache_dir_at(root).join(format!("{}-{}.{}", name, version, ext))
}

pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [packages_dir(), data_dir(), db_dir(), log_dir(), cache_dir()] {
        fs::create_dir_all(dir)?;
    }

    Ok(())
}

pub fn ensure_install_dirs(root: &Path) -> std::io::Result<()> {
    let dir = packages_dir_at(root);
    fs::create_dir_all(dir)?;

    Ok(())
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
        .unwrap_or_else(base_dir)
}
