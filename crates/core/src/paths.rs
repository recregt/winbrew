//! Managed-root path helpers and derived directory contracts.
//!
//! The path layer owns the active WinBrew root and the directories derived from
//! it. That includes the current runtime database layout, per-package journal
//! paths, package-scoped evidence directories, and reserved shim locations.

use std::path::{Path, PathBuf};
/// Return the journal file for the given package key.
#[cfg(windows)]
use winbrew_windows::host::search_path_file;

/// Fully resolved path set for the active WinBrew root.
#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    /// Managed root directory.
    pub root: PathBuf,
    /// Install root for packages.
    pub packages: PathBuf,
    /// Root data directory.
    pub data: PathBuf,
    /// Process log directory.
    pub logs: PathBuf,
    /// Package-scoped log parent directory.
    pub package_logs: PathBuf,
    /// Download and staging cache directory.
    pub cache: PathBuf,
    /// Recovery journal parent directory.
    pub pkgdb: PathBuf,
    /// Reserved package shim root.
    pub shims: PathBuf,
    /// Primary SQLite database directory.
    pub db: PathBuf,
    /// Catalog database path.
    pub catalog_db: PathBuf,
    /// Persisted configuration file.
    pub config: PathBuf,
    /// Process log file.
    pub log: PathBuf,
}

#[derive(Debug, Clone)]
struct ManagedRootLayout {
    root: PathBuf,
    packages: PathBuf,
    data: PathBuf,
    logs: PathBuf,
    package_logs: PathBuf,
    cache: PathBuf,
    pkgdb: PathBuf,
    shims: PathBuf,
    db: PathBuf,
    catalog_db: PathBuf,
    config: PathBuf,
    log: PathBuf,
}

impl ManagedRootLayout {
    fn resolve(root: &Path, packages: &str, data: &str, logs: &str, cache: &str) -> Self {
        let root = PathBuf::from(root);
        let packages = resolve_template(&root, packages);
        let data = resolve_template(&root, data);
        let logs = resolve_template(&root, logs);
        let cache = resolve_template(&root, cache);
        let pkgdb = data.join("pkgdb");
        let package_logs = logs.join("packages");
        let shims = root.join("shims");
        let db = data.join("db");

        Self {
            catalog_db: db.join("catalog.db"),
            config: data.join("winbrew.toml"),
            db: db.join("winbrew.db"),
            log: logs.join("winbrew.log"),
            package_logs,
            packages,
            pkgdb,
            root,
            cache,
            data,
            logs,
            shims,
        }
    }

    fn into_resolved_paths(self) -> ResolvedPaths {
        ResolvedPaths {
            root: self.root,
            packages: self.packages,
            data: self.data,
            logs: self.logs,
            package_logs: self.package_logs,
            cache: self.cache,
            pkgdb: self.pkgdb,
            shims: self.shims,
            db: self.db,
            catalog_db: self.catalog_db,
            config: self.config,
            log: self.log,
        }
    }
}

/// Return the persisted configuration file for a root.
pub fn config_file_at(root: &Path) -> PathBuf {
    root.join("data").join("winbrew.toml")
}

/// Return the install root directory for a root.
pub fn packages_dir_at(root: &Path) -> PathBuf {
    root.join("packages")
}

/// Return the data directory for a root.
pub fn data_dir_at(root: &Path) -> PathBuf {
    root.join("data")
}

/// Return the package journal directory for a root.
pub fn pkgdb_dir_at(root: &Path) -> PathBuf {
    data_dir_at(root).join("pkgdb")
}

/// Return the SQLite database directory for a root.
pub fn db_dir_at(root: &Path) -> PathBuf {
    data_dir_at(root).join("db")
}

/// Return the primary SQLite database path for a root.
pub fn db_path_at(root: &Path) -> PathBuf {
    db_dir_at(root).join("winbrew.db")
}

/// Return the catalog database path for a root.
pub fn catalog_db_at(root: &Path) -> PathBuf {
    db_dir_at(root).join("catalog.db")
}

/// Return the process log directory for a root.
pub fn log_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("logs")
}

/// Return the process log file for a root.
pub fn log_file_at(root: &Path) -> PathBuf {
    log_dir_at(root).join("winbrew.log")
}

/// Return the installer cache directory for a root.
pub fn cache_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("cache")
}

/// Return a cache file path for the given package name and version.
pub fn cache_file_at(root: &Path, name: &str, version: &str, ext: &str) -> PathBuf {
    cache_dir_at(root).join(cache_filename(name, version, ext))
}

/// Return the 7-Zip runtime directory for a managed root.
pub fn sevenz_runtime_dir_from_runtime_root(runtime_root: &Path) -> PathBuf {
    runtime_root.join("bin/7zip")
}

/// Return the 7-Zip binary path for a managed root.
pub fn sevenz_bin_path_from_runtime_root(runtime_root: &Path) -> PathBuf {
    sevenz_runtime_dir_from_runtime_root(runtime_root).join("7z.exe")
}

/// Return the 7-Zip DLL path for a managed root.
pub fn sevenz_dll_path_from_runtime_root(runtime_root: &Path) -> PathBuf {
    sevenz_runtime_dir_from_runtime_root(runtime_root).join("7z.dll")
}

/// Return the first usable 7-Zip binary found on the current PATH.
#[cfg(windows)]
pub fn system_sevenz_binary_path() -> Option<PathBuf> {
    search_path_file("7z.exe").and_then(|binary_path| {
        let runtime_root = binary_path.parent()?;

        if runtime_root.join("7z.dll").exists() {
            Some(binary_path)
        } else {
            None
        }
    })
}

/// Return the first usable 7-Zip binary found on the current PATH.
#[cfg(not(windows))]
pub fn system_sevenz_binary_path() -> Option<PathBuf> {
    None
}

/// Return the journal file for the given package key.
pub fn package_journal_file_at(root: &Path, package_key: &str) -> PathBuf {
    pkgdb_dir_at(root).join(package_key).join("journal.jsonl")
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

/// Expand `${root}` placeholders inside a path template.
pub fn resolve_template(root: &Path, template: &str) -> PathBuf {
    let root_text = root.to_string_lossy();

    if template.contains("${root}") {
        PathBuf::from(template.replace("${root}", &root_text))
    } else {
        PathBuf::from(template)
    }
}

/// Build the resolved path set for the active root layout.
pub fn resolved_paths(
    root: &Path,
    packages: &str,
    data: &str,
    logs: &str,
    cache: &str,
) -> ResolvedPaths {
    ManagedRootLayout::resolve(root, packages, data, logs, cache).into_resolved_paths()
}

impl ResolvedPaths {
    /// Return the install directory for a package name.
    pub fn package_install_dir(&self, package_name: &str) -> PathBuf {
        self.packages.join(package_name)
    }

    /// Return the journal directory for a package key.
    pub fn package_journal_dir(&self, package_key: &str) -> PathBuf {
        self.pkgdb.join(package_key)
    }

    /// Return the journal file for a package key.
    pub fn package_journal_file(&self, package_key: &str) -> PathBuf {
        self.package_journal_dir(package_key).join("journal.jsonl")
    }

    /// Return the package-scoped log directory for a package key.
    pub fn package_log_dir(&self, package_key: &str) -> PathBuf {
        self.package_logs.join(package_key)
    }

    /// Return the reserved shim directory for a package key.
    pub fn package_shim_dir(&self, package_key: &str) -> PathBuf {
        self.shims.join(package_key)
    }
}

/// Recover the managed root from a package install directory.
pub fn install_root_from_package_dir(install_dir: &Path) -> PathBuf {
    install_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        cache_dir_at, catalog_db_at, config_file_at, data_dir_at, db_path_at, log_dir_at,
        log_file_at, package_journal_file_at, packages_dir_at, pkgdb_dir_at, resolved_paths,
        sevenz_bin_path_from_runtime_root, sevenz_dll_path_from_runtime_root,
        sevenz_runtime_dir_from_runtime_root,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn package_journal_file_lives_under_pkgdb() {
        let root = tempdir().expect("temp dir");
        let package_key = "winget_Contoso.App-c47f5b18b8a430e6";

        let journal_file = package_journal_file_at(root.path(), package_key);

        assert_eq!(
            journal_file,
            pkgdb_dir_at(root.path())
                .join(package_key)
                .join("journal.jsonl")
        );
    }

    #[test]
    fn sevenz_runtime_layout_uses_expected_relative_paths() {
        let runtime_root = PathBuf::from("C:/winbrew");

        assert_eq!(
            sevenz_runtime_dir_from_runtime_root(&runtime_root),
            PathBuf::from("C:/winbrew/bin/7zip")
        );
        assert_eq!(
            sevenz_bin_path_from_runtime_root(&runtime_root),
            PathBuf::from("C:/winbrew/bin/7zip/7z.exe")
        );
        assert_eq!(
            sevenz_dll_path_from_runtime_root(&runtime_root),
            PathBuf::from("C:/winbrew/bin/7zip/7z.dll")
        );
    }

    #[test]
    fn resolved_paths_derive_managed_layout_and_package_scopes() {
        let root = tempdir().expect("temp dir");
        let package_key = "winget_Contoso.App-c47f5b18b8a430e6";
        let paths = resolved_paths(
            root.path(),
            "${root}\\packages",
            "${root}\\data",
            "${root}\\data\\logs",
            "${root}\\data\\cache",
        );

        assert_eq!(paths.root, root.path());
        assert_eq!(paths.packages, packages_dir_at(root.path()));
        assert_eq!(
            paths.package_install_dir("Contoso.App"),
            paths.packages.join("Contoso.App")
        );
        assert_eq!(paths.data, data_dir_at(root.path()));
        assert_eq!(paths.logs, log_dir_at(root.path()));
        assert_eq!(paths.package_logs, paths.logs.join("packages"));
        assert_eq!(paths.cache, cache_dir_at(root.path()));
        assert_eq!(paths.pkgdb, pkgdb_dir_at(root.path()));
        assert_eq!(paths.shims, root.path().join("shims"));
        assert_eq!(paths.db, db_path_at(root.path()));
        assert_eq!(paths.catalog_db, catalog_db_at(root.path()));
        assert_eq!(paths.config, config_file_at(root.path()));
        assert_eq!(paths.log, log_file_at(root.path()));
        assert_eq!(
            paths.package_journal_dir(package_key),
            paths.pkgdb.join(package_key)
        );
        assert_eq!(
            paths.package_journal_file(package_key),
            package_journal_file_at(root.path(), package_key)
        );
        assert_eq!(
            paths.package_log_dir(package_key),
            paths.package_logs.join(package_key)
        );
        assert_eq!(
            paths.package_shim_dir(package_key),
            paths.shims.join(package_key)
        );
    }
}
