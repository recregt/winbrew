//! Shared environment variable names used by Winbrew's path and config resolution.
//!
//! These constants define the public environment-variable names that higher-level
//! configuration code reads before it resolves the final application paths.
//! They are stable strings that external scripts, tests, and administrators can
//! rely on when overriding Winbrew's defaults.
//!
//! # Platform Notes
//!
//! `LOCALAPPDATA` is a Windows-specific convention. Winbrew exports the name so
//! the default Windows application root can be derived consistently.
//!
//! # Security Considerations
//!
//! These values are read from the process environment via `std::env::var`.
//! Treat them as untrusted input until the caller validates or normalizes the
//! resulting path.

#[doc(alias = "appdata")]
#[doc(alias = "local appdata")]
/// Standard Windows environment variable that points to the user's Local AppData directory.
///
/// Winbrew uses this as the base directory for the default application root when
/// `WINBREW_PATHS_ROOT` is not set. The resolved path typically follows:
/// `%LOCALAPPDATA%\winbrew`.
pub const LOCALAPPDATA: &str = "LOCALAPPDATA";

#[doc(alias = "root")]
#[doc(alias = "install_dir")]
#[doc(alias = "paths.root")]
/// Winbrew-specific environment variable that overrides the configured `paths.root` value.
///
/// When set, this bypasses the persisted `paths.root` setting and the default
/// `LOCALAPPDATA`-based root. Tests, CI jobs, and portable runs can point Winbrew
/// at an alternate root directory without changing user configuration.
pub const WINBREW_PATHS_ROOT: &str = "WINBREW_PATHS_ROOT";
