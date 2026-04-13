//! Package identity and query models.
//!
//! This module owns the type-safe representations used when a caller wants to
//! talk about a package independent of storage, catalog payload shape, or
//! installed state. The types here bridge user-facing package references,
//! package aggregates, and dependency/query helpers.
//!
//! Use this module when you need:
//!
//! - a canonical package aggregate (`Package`)
//! - a package source/kind classification
//! - package reference parsing (`PackageRef`, `PackageId`)
//! - a text query model for search flows (`PackageQuery`)
//! - package dependency metadata (`Dependency`)

pub mod dependency;
pub mod model;
pub mod query;
pub mod reference;

pub use dependency::Dependency;
pub use model::{Package, PackageKind, PackageSource};
pub use query::PackageQuery;
pub use reference::{PackageId, PackageRef};
