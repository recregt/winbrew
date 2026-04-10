//! App-facing search facade over the catalog search.
//!
//! This module keeps the command boundary stable while the catalog layer
//! owns the actual search error semantics.

use crate::install_crate::catalog;
use crate::models::CatalogPackage;

pub use catalog::{SearchError, SearchResult};

pub fn search_packages(query: &str) -> SearchResult<Vec<CatalogPackage>> {
    catalog::search_packages(query)
}
