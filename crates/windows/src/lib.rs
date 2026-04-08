#![allow(missing_docs)]

mod registry;
mod uninstall;

pub use registry::{AppInfo, collect_installed_apps};
pub use uninstall::{Hive, UninstallRoot, uninstall_roots};
