//! Core utilities shared by Winbrew.
//!
//! ## Environment Configuration
//!
//! The core crate exposes the Windows environment variable names that higher-level
//! code uses while resolving paths.
//!
//! ```no_run
//! use winbrew_core::env;
//!
//! if let Ok(custom_root) = std::env::var(env::WINBREW_PATHS_ROOT) {
//!     println!("Using custom root: {custom_root}");
//! }
//! ```
//!
//! When `WINBREW_PATHS_ROOT` is not set, the application root is typically derived
//! from [`env::LOCALAPPDATA`] and expanded into the resolved path set in
//! [`crate::paths::ResolvedPaths`].

pub mod env;
pub mod fs;
pub mod hash;
pub mod network;
pub mod paths;
pub mod time;

pub use env::{LOCALAPPDATA, WINBREW_PATHS_ROOT};
pub use fs::{
    atomic_write, atomic_write_with_pid_suffix, backup_directory_path, cleanup_path,
    extract_zip_archive, finalize_temp_file, replace_directory,
};
pub use hash::{
    HashAlgorithm, HashError, Hasher, Result as HashResult, hash_algorithm, normalize_hash,
    verify_hash,
};
pub use network::{
    Client, build_client, download_url_to_temp_file, installer_filename, is_zip_path,
};
pub use paths::{
    ResolvedPaths, cache_dir_at, cache_file_at, catalog_db_at, config_file_at, data_dir_at,
    db_dir_at, db_path_at, ensure_dirs_at, ensure_install_dirs_at, install_root_from_package_dir,
    log_dir_at, log_file_at, resolve_template, resolved_paths,
};
pub use time::{now, now_ms};
