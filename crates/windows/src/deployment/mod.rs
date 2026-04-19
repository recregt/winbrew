mod msi;
mod msix;

pub use msi::scan_inventory as msi_scan_inventory;
pub use msix::{
    install as msix_install, installed_package_full_name as msix_installed_package_full_name,
    remove as msix_remove,
};
