//! Windows platform boundary for `winbrew-engines`.
//!
//! This module exists so WinBrew can keep operating-system-specific behavior
//! in one place instead of scattering `cfg(windows)` checks throughout the
//! engine registry and crate root.
//!
//! WinBrew has four different Windows-facing responsibilities:
//!
//! - [`native`] launches installers as processes, which covers MSI and native
//!   executable families.
//! - [`font`] installs and removes per-user Windows fonts.
//! - [`api`] delegates to Windows package APIs, which currently covers MSIX.
//! - the rest of the crate stays platform-neutral and only calls into this
//!   layer when the selected engine needs Windows-specific behavior.
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
//! - [`native`] for process-driven installer backends such as MSI and native
//!   `.exe` installers
//! - [`font`] for per-user Windows font installation and removal
//! - [`api`] for Windows package API adapters such as MSIX
//!
//! Example: pick the backend family without reaching into the lower-level
//! implementation details.
//!
//! ```rust,no_run
//! use winbrew_engines::windows::{api, native};
//!
//! #[cfg(windows)]
//! fn choose_backend() {
//!     let _ = api::msix::install;
//!     let _ = native::exe::install;
//! }
//! ```

pub mod font;
#[cfg(windows)]
pub mod native;

pub mod api;
