//! Typed catalog payloads and raw upstream catalog records.
//!
//! The catalog family separates Winbrew's typed catalog surface from the raw
//! schema that comes from upstream ingestion. The typed types validate package
//! identity, source, and installer metadata; the raw types intentionally stay
//! schema-shaped so conversion code can own parsing and normalization.
//!
//! Keep catalog-specific logic here when it needs to answer one of these
//! questions:
//!
//! - what a catalog package looks like after validation
//! - how a raw package or installer record should be converted
//! - which metadata fields belong to the generated catalog index

pub mod conversion;
pub mod installer_type;
pub mod metadata;
pub mod package;
pub mod raw;

pub use installer_type::CatalogInstallerType;
pub use metadata::CatalogMetadata;
pub use package::{CatalogInstaller, CatalogPackage};
pub use raw::{RawCatalogInstaller, RawCatalogPackage};
