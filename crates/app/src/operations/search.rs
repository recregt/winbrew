//! App-facing search facade over the catalog search.
//!
//! This module keeps the command boundary stable while the catalog layer
//! owns the actual search error semantics.

use crate::catalog;
use crate::database;
use crate::models::domains::catalog::CatalogPackage;
use anyhow::Error;

#[derive(Debug)]
pub enum SearchError {
    CatalogUnavailable,
    Unexpected(Error),
}

pub type SearchResult<T> = std::result::Result<T, SearchError>;

pub fn search_packages(query: &str) -> SearchResult<Vec<CatalogPackage>> {
    match catalog::search_packages(query) {
        Ok(packages) => Ok(packages),
        Err(err)
            if err
                .downcast_ref::<database::CatalogNotFoundError>()
                .is_some() =>
        {
            Err(SearchError::CatalogUnavailable)
        }
        Err(err) => Err(SearchError::Unexpected(err)),
    }
}
