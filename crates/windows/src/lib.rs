#![cfg(windows)]
#![doc = include_str!("../README.md")]

mod deployment;
mod fs;
mod registry;

pub use deployment::{
    msi_scan_inventory, msix_install, msix_installed_package_full_name, msix_remove,
};
pub use fs::{PathInfo, create_extracted_file, inspect_path};
pub use registry::{
    AppInfo, Hive, UninstallEntryGuard, UninstallRoot, collect_installed_apps,
    create_test_uninstall_entry, uninstall_roots, uninstall_value,
};
