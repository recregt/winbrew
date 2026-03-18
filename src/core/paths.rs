use std::env;
use std::fs;
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

pub fn package_dir(name: &str) -> PathBuf {
    base_dir().join("packages").join(name)
}

pub fn bin_dir() -> PathBuf {
    base_dir().join("bin")
}

pub fn data_dir() -> PathBuf {
    base_dir().join("data")
}

pub fn db_path() -> PathBuf {
    base_dir().join("data").join("winbrew.db")
}

pub fn cache_dir() -> PathBuf {
    base_dir().join("cache")
}

pub fn cache_file(name: &str, version: &str, ext: &str) -> PathBuf {
    base_dir().join("cache").join(format!("{}-{}.{}", name, version, ext))
}

pub fn shim_path(name: &str) -> PathBuf {
    base_dir().join("bin").join(format!("{}.shim", name))
}

pub fn ensure_dirs() -> std::io::Result<()> {
    let base = base_dir(); // Read from global cache
    
    // Create all directories using the cached base path
    for dir in ["packages", "bin", "data", "cache"] {
        fs::create_dir_all(base.join(dir))?;
    }
    
    Ok(())
}