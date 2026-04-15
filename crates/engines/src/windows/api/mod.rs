//! Public Windows API facade for `winbrew-engines`.
//!
//! This layer keeps the Windows-facing surface narrow and documented. The
//! only public adapter today is `msix`, and its leaf modules stay private so
//! the crate exposes a small, stable entry point.

pub mod msix;
