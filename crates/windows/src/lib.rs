#![allow(missing_docs)]

mod filesystem;
mod registry;
#[cfg(windows)]
mod uninstall;
#[cfg(not(windows))]
mod uninstall {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Hive {
        LocalMachine,
        CurrentUser,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct UninstallRoot {
        pub hive: Hive,
        pub key_path: &'static str,
        pub label: &'static str,
    }

    pub fn uninstall_roots() -> std::iter::Empty<UninstallRoot> {
        std::iter::empty()
    }
}

pub use filesystem::{PathInfo, create_extracted_file, inspect_path};
pub use registry::{AppInfo, collect_installed_apps};
pub use uninstall::{Hive, UninstallRoot, uninstall_roots};
