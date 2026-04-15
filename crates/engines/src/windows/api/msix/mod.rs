//! MSIX adapter surface used by the Windows API facade.
//!
//! What this module does:
//!
//! - installs MSIX packages by delegating to the Windows App Installer APIs
//! - removes MSIX packages using the package full name stored in the receipt
//! - keeps the leaf implementation modules private so callers only see the
//!   narrow `install` and `remove` entry points
//!
//! What this module does not do:
//!
//! - it does not extract archives or copy payload files
//! - it does not infer package identity from the registry
//! - it does not own the package layout logic for portable or zip-based flows

mod install;
mod remove;

/// Install an MSIX package through the Windows App Installer APIs.
///
/// The function delegates the actual package registration to Windows, creates
/// the target install directory, and returns an `EngineInstallReceipt` that
/// stores the MSIX package full name and install scope.
pub use install::install;
/// Remove an MSIX package using the package full name stored in the receipt.
///
/// The function expects `EngineMetadata::Msix` to be present on the installed
/// package and delegates the uninstall operation to Windows.
pub use remove::remove;
