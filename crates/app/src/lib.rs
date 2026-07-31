//! Workflow layer for WinBrew.
//!
//! `winbrew-app` owns the business-level orchestration for install, update,
//! doctor, repair, and related command flows. It sits between the CLI
//! presentation layer and the lower-level core, database, engines, and models
//! crates, so it can keep execution logic reusable in tests and other callers.

pub mod context;

pub use winbrew_core as core;
pub use winbrew_database as database;
pub use winbrew_engines as engines;
pub use winbrew_models as models;

#[cfg(windows)]
pub use winbrew_windows as windows;

#[cfg(not(windows))]
pub mod windows {
    pub mod host {
        use crate::models::domains::install::Architecture;

        const NORMAL_PLATFORM_TAGS: &[&str] =
            &["windows.desktop", "windows.ltsc", "windows.universal"];
        const SERVER_PLATFORM_TAGS: &[&str] = &["windows.server"];

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct HostProfile {
            pub is_server: bool,
            pub architecture: Architecture,
        }

        impl HostProfile {
            pub fn platform_tags(self) -> &'static [&'static str] {
                if self.is_server {
                    SERVER_PLATFORM_TAGS
                } else {
                    NORMAL_PLATFORM_TAGS
                }
            }
        }

        impl std::fmt::Display for HostProfile {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let family = if self.is_server { "server" } else { "desktop" };
                write!(f, "{family} {}", architecture_label(self.architecture))
            }
        }

        pub fn architecture_label(architecture: Architecture) -> &'static str {
            match architecture {
                Architecture::X64 => "x64",
                Architecture::X86 => "x86",
                Architecture::Arm64 => "arm64",
                Architecture::Any => "any",
            }
        }

        pub fn host_profile() -> HostProfile {
            HostProfile {
                is_server: false,
                architecture: Architecture::Any,
            }
        }

        pub fn windows_version_string() -> Option<String> {
            None
        }

        pub fn is_elevated() -> bool {
            false
        }
    }

    pub mod installed {
        use anyhow::Result;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct UninstallEntry {
            pub display_name: String,
        }

        pub fn uninstall_entries_matching(_name: &str) -> Result<Vec<UninstallEntry>> {
            Ok(Vec::new())
        }
    }
}

mod catalog;
pub mod operations;

pub use context::AppContext;
pub use operations::{
    config, doctor, info, install, list, remove, repair, report, search, update, version,
};
