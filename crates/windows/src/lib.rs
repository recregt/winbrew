#![allow(missing_docs)]

mod filesystem;
mod registry;
mod uninstall;

pub use filesystem::{PathInfo, inspect_path};
pub use registry::{AppInfo, collect_installed_apps};
pub use uninstall::{Hive, UninstallRoot, uninstall_roots};
