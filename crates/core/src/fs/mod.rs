//! File-system helpers used by Winbrew's configuration, download, and install flows.
//!
//! This module defines the public filesystem API and keeps the implementation
//! split across smaller internal modules.

mod archive;
mod cleanup;
mod move_or_copy;
mod write;

pub use archive::extract_zip_archive;
pub use cleanup::cleanup_path;
pub use move_or_copy::{backup_directory_path, replace_directory};
pub use write::{atomic_write, atomic_write_with_pid_suffix, finalize_temp_file};
