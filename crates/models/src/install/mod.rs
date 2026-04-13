//! Installer metadata, engine receipts, installed state, and removal planning.
//!
//! This module covers the lifecycle surface for installation and removal. It
//! owns the engine-facing receipts, the persisted installed record, the
//! installer and architecture classifiers, and the outcome/failure types that
//! higher layers use to report install progress.
//!
//! Prefer the `engine` submodule for engine kind and scope, `installer` for
//! packaging format metadata, `installed` for persisted package state, and
//! `model` for success/failure results.

pub mod engine;
pub mod installed;
pub mod installer;
pub mod model;
pub mod removal;

pub use engine::{EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope};
pub use installed::{InstalledPackage, PackageStatus};
pub use installer::{Architecture, Installer, InstallerType};
pub use model::{InstallFailureClass, InstallOutcome, InstallResult};
pub use removal::RemovalPlan;
