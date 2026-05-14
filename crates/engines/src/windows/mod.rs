//! Windows platform boundary for `winbrew-engines`.
//!
//! This module keeps all Windows-specific engine backends at one level so the
//! public surface stays direct: `exe`, `font`, `msi`, and `msix`.
//!
//! Why this layer is useful:
//!
//! - it keeps Windows-only dependencies and process/registry logic out of the
//!   routing code
//! - it makes the ownership split obvious: filesystem engines live elsewhere,
//!   Windows-delegated engines live here
//! - it gives `cargo doc` a single place to explain the Windows backend shape
//!   and the public entry points
//!
//! What to read next:
//!
//! - [`exe`] for process-driven installer backends such as native `.exe`
//!   installers
//! - [`font`] for per-user Windows font installation and removal
//! - [`msi`] for Windows Installer packages
//! - [`msix`] for Windows package integration such as MSIX
//!
//! Example: pick the backend family without reaching into a nested namespace.
//!
//! ```rust,no_run
//! use winbrew_engines::windows::{exe, msix};
//!
//! #[cfg(windows)]
//! fn choose_backend() {
//!     let _ = msix::install;
//!     let _ = exe::install;
//! }
//! ```

#[cfg(windows)]
pub mod exe;
pub mod font;
#[cfg(windows)]
pub mod msi;

pub mod msix;
