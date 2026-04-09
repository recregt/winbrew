#![cfg(windows)]
#![allow(missing_docs)]

mod filesystem;
mod registry;
mod uninstall;

pub use filesystem::PathInfo;
pub use filesystem::{create_extracted_file, inspect_path};
pub use registry::{AppInfo, collect_installed_apps};
pub use uninstall::{Hive, UninstallRoot, uninstall_roots};
