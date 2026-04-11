#![cfg(windows)]
#![doc = include_str!("../README.md")]

mod deployment;
mod fs;
mod registry;

pub use deployment::{
    msi_scan_inventory, msix_install, msix_installed_package_full_name, msix_remove,
};
pub use fs::{PathInfo, create_extracted_file, inspect_path};
pub use registry::{AppInfo, Hive, UninstallRoot, collect_installed_apps, uninstall_roots};
