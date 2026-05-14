//! Native executable installation and removal for Windows.
//!
//! This backend handles installer families that are executed as processes
//! rather than unpacked as files:
//!
//! - generic native `.exe` installers when explicit switches are provided
//! - Inno Setup installers
//! - Nullsoft / NSIS installers
//! - Burn bootstrapper installers
//!
//! What this module does:
//!
//! - validates the installer path, install directory, and package name before
//!   starting work
//! - parses installer switches literally and rejects duplicate installer
//!   switches before execution, so catalog mistakes fail fast instead of being
//!   silently normalized
//! - launches the downloaded installer as a process and treats the Windows
//!   installer success codes `0`, `1641`, and `3010` as successful outcomes
//! - captures uninstall metadata from the Windows uninstall registry when it
//!   can, so later removal can reuse the recorded command
//! - falls back to direct directory cleanup when uninstall metadata is missing
//!   or the uninstall command fails
//!
//! What this module does not do:
//!
//! - it does not extract archives or copy payload files
//! - it does not infer installer family from URLs alone
//! - it does not own MSIX / App Installer behavior, which lives in the MSIX
//!   module

mod install;
mod metadata;
mod remove;
mod switches;
mod validation;

#[cfg(test)]
mod tests;

pub use install::install;
pub use remove::remove;

pub(super) const NATIVE_EXE_SUCCESS_EXIT_CODES: &[i32] = &[0, 1641, 3010];
