#![cfg(windows)]
#![doc = include_str!("../README.md")]

mod deployment;
mod font;
mod fs;
mod registry;
mod system;

pub use deployment::{
    msi_scan_inventory, msix_install, msix_installed_package_full_name, msix_remove,
};
pub use font::{install_user_font, remove_user_font, user_fonts_dir};
pub use fs::{PathInfo, create_extracted_file, inspect_path};
pub use registry::{
    AppInfo, UninstallEntry, UninstallEntryGuard, collect_installed_apps,
    collect_uninstall_entries, create_test_uninstall_entry,
    create_test_uninstall_entry_with_install_location, uninstall_value,
};
pub use system::{HostProfile, host_profile, is_elevated};
