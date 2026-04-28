#![cfg(windows)]
#![doc = include_str!("../README.md")]

mod deployment;
mod font;
mod fs;
mod registry;
mod system;

pub(crate) use winbrew_models as models;

/// Windows uninstall registry helpers for listing installed applications.
pub mod apps {
    pub use crate::registry::{
        AppInfo, UninstallEntry, collect_installed_apps, collect_uninstall_entries, uninstall_value,
    };
}

/// System architecture, privilege, PATH, and Windows version helpers.
pub mod host {
    pub use crate::system::{
        HostProfile, host_profile, is_elevated, search_path_file, windows_version_string,
    };
}

/// User font install and removal helpers.
pub mod fonts {
    pub use crate::font::{install_user_font, remove_user_font, user_fonts_dir};
}

/// MSI and MSIX package helpers.
pub mod packages {
    pub use crate::deployment::{
        msi_scan_inventory, msix_install, msix_installed_package_full_name, msix_remove,
    };
}

/// Filesystem inspection and extraction helpers.
pub mod paths {
    pub use crate::fs::{PathInfo, create_extracted_file, inspect_path};
}

/// Test-only registry helpers.
#[cfg(any(test, feature = "testing"))]
pub mod testing {
    pub use crate::host::windows_version_string;
    pub use crate::registry::{
        UninstallEntryGuard, create_test_uninstall_entry,
        create_test_uninstall_entry_with_install_location,
    };
}
