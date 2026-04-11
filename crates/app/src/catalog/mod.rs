//! Catalog search and installer selection facade.

// The catalog module is an internal helper layer. Public entry points live in operations.

mod search;
mod select;

pub(crate) use search::{resolve_catalog_package_ref, search_packages};
pub(crate) use select::select_installer;
