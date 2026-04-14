//! Internal catalog facade for package search and installer selection.
//!
//! The app crate keeps catalog behavior behind this module so command-facing
//! code can work with resolved packages instead of raw database queries. The
//! public-facing command APIs live in `operations`; this module only provides
//! the catalog-specific plumbing needed by those workflows.
//!
//! Responsibilities are split into two focused submodules:
//!
//! - `search` resolves package references and interactive package queries.
//! - `select` chooses the best installer for the current machine architecture.
//!
//! Keeping these concerns together makes the catalog rules easy to audit while
//! still leaving the CLI layer unaware of database and ranking details.

mod search;
mod select;

pub(crate) use search::{resolve_catalog_package_ref, search_packages};
pub(crate) use select::select_installer;
