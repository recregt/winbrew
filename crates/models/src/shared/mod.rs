//! Foundational types shared across every model family.
//!
//! This module owns the low-level contracts that do not belong to a specific
//! package, catalog, install, or reporting concept. Keep the types here small,
//! strongly typed, and dependency-light because many higher-level records use
//! them transitively.
//!
//! The most important sub-areas are:
//!
//! - `config`: configuration sections and value provenance
//! - `error`: the canonical `ModelError` used by parse and validation code
//! - `deployment`: deployment outcome metadata shared by install and reporting code
//! - `hash`: checksum algorithm metadata and legacy algorithm detection
//! - `identifiers`: strongly typed package/catalog identifiers
//! - `validation`: the shared `Validate` trait and helper functions
//! - `version`: semver-backed version parsing and normalization

pub mod config;
pub mod deployment;
pub mod error;
pub mod hash;
pub mod identifiers;
pub mod validation;
pub mod version;

pub use config::{ConfigSection, ConfigValue, ConfigValueSource};
pub use deployment::DeploymentKind;
pub use error::ModelError;
pub use hash::HashAlgorithm;
pub use identifiers::{BucketName, CatalogId, PackageName};
pub use validation::Validate;
pub use version::Version;
