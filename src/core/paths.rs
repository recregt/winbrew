use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

const DEFAULT_ROOT: &str = r"C:\winbrew";

// Calculates the base directory exactly ONCE and caches it globally.
pub fn base_dir() -> &'static PathBuf {
    static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

    BASE_DIR.get_or_init(|| {
        env::var("WINBREW_ROOT")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT))
    })
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
        .unwrap_or_else(|| base_dir().clone())
}

pub fn package_dir(name: &str) -> PathBuf {
    base_dir().join("packages").join(name)
}

pub fn package_dir_at(root: &Path, name: &str) -> PathBuf {
    packages_dir_at(root).join(name)
}

pub fn bin_dir() -> PathBuf {
    base_dir().join("bin")
}

pub fn bin_dir_at(root: &Path) -> PathBuf {
    root.join("bin")
}

pub fn data_dir() -> PathBuf {
    base_dir().join("data")
}

pub fn db_path() -> PathBuf {
    base_dir().join("data").join("winbrew.db")
}

pub fn config_file() -> PathBuf {
    base_dir().join("data").join("winbrew.toml")
}

pub fn log_dir() -> PathBuf {
    base_dir().join("data").join("logs")
}

pub fn log_file() -> PathBuf {
    log_dir().join("winbrew.log")
}

pub fn cache_dir() -> PathBuf {
    base_dir().join("cache")
}

pub fn cache_dir_at(root: &Path) -> PathBuf {
    root.join("cache")
}

pub fn cache_file(name: &str, version: &str, ext: &str) -> PathBuf {
    cache_dir().join(format!("{}-{}.{}", name, version, ext))
}

pub fn cache_file_at(root: &Path, name: &str, version: &str, ext: &str) -> PathBuf {
    cache_dir_at(root).join(format!("{}-{}.{}", name, version, ext))
}

pub fn shim_path(name: &str) -> PathBuf {
    bin_dir().join(format!("{}.shim", name))
}

pub fn shim_path_at(root: &Path, name: &str) -> PathBuf {
    bin_dir_at(root).join(format!("{}.shim", name))
}

pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [
        packages_dir(),
        bin_dir(),
        data_dir(),
        log_dir(),
        cache_dir(),
    ] {
        fs::create_dir_all(dir)?;
    }

    Ok(())
}

pub fn ensure_install_dirs(root: &Path) -> std::io::Result<()> {
    for dir in [packages_dir_at(root), bin_dir_at(root)] {
        fs::create_dir_all(dir)?;
    }

    Ok(())
}

pub fn install_root_from_package_dir(install_dir: &Path) -> PathBuf {
    install_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| base_dir().clone())
}
