//! App-facing search facade over shared catalog search.
//!
//! This module keeps the command boundary stable while the shared catalog layer
//! owns the actual search error semantics.

use crate::services::shared::catalog;
use winbrew_models::CatalogPackage;

pub use catalog::{SearchError, SearchResult};

pub fn search_packages(query: &str) -> SearchResult<Vec<CatalogPackage>> {
    catalog::search_packages(query)
}
