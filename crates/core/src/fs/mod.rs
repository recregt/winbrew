//! File-system helpers used by Winbrew's configuration, download, and install flows.
//!
//! This module provides atomic, transactional filesystem operations with
//! Windows-specific security considerations such as reparse points and hard
//! links.
//!
//! # Operations
//!
//! | Operation | Purpose | Atomicity |
//! |-----------|---------|-----------|
//! | [`cleanup_path`] | Remove files and directories with deferred-delete fallback | Best-effort |
//! | [`replace_directory`] | Swap directories with rollback and cross-volume fallback | Transactional |
//! | [`atomic_write`] | Write files atomically via temp file + rename | All-or-nothing |
//! | [`extract_zip_archive`] | Extract ZIPs with path traversal protection | Rollback on fail |
//!
//! # Platform Behavior
//!
//! - **Windows**: Handles junction points, hard links, and cross-volume copies.
//! - **Unix**: Standard POSIX semantics; reparse-point checks are no-ops.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use winbrew_core::fs::{atomic_write, cleanup_path, replace_directory};
//!
//! atomic_write(
//!     Path::new("config.toml"),
//!     Path::new("config.toml.tmp"),
//!     b"key = value",
//! ).map_err(|err| *err)?;
//!
//! replace_directory(Path::new("staging/app"), Path::new("app")).map_err(|err| *err)?;
//! cleanup_path(Path::new("app.old")).map_err(|err| *err)?;
//! # Ok::<(), winbrew_core::fs::FsError>(())
//! ```

mod archive;
mod cleanup;
mod error;
mod move_or_copy;
mod write;

pub use archive::extract_zip_archive;
pub use cleanup::cleanup_path;
pub use error::{FsError, Result};
pub use move_or_copy::{backup_path_for, replace_directory};
pub use write::{atomic_write, atomic_write_toml_temp, finalize_temp_file};
