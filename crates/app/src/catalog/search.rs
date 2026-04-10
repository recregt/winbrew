//! Catalog search helpers for resolving packages by query or package reference.

use anyhow::Result;

use super::error::{SearchError, SearchResult};
use crate::storage;
use winbrew_models::{CatalogPackage, PackageRef};

/// Searches the catalog using an already-open catalog database connection.
///
/// This helper is for callers that already resolved the catalog connection and
/// want a plain `anyhow::Result` for query-level failures only.
pub fn search_catalog_packages(
    conn: &storage::DbConnection,
    query: &str,
) -> Result<Vec<CatalogPackage>> {
    storage::search(conn, query)
}

/// Searches the shared catalog connection, returning `CatalogUnavailable` when the database is missing.
pub fn search_packages(query: &str) -> SearchResult<Vec<CatalogPackage>> {
    let conn = match storage::get_catalog_conn() {
        Ok(conn) => conn,
        Err(err)
            if err
                .downcast_ref::<storage::CatalogNotFoundError>()
                .is_some() =>
        {
            return Err(SearchError::CatalogUnavailable);
        }
        Err(err) => return Err(SearchError::Unexpected(err)),
    };

    match storage::search(&conn, query) {
        Ok(packages) => Ok(packages),
        Err(err) => Err(SearchError::Unexpected(err)),
    }
}

/// Resolves a package from a query, preferring exact name matches before asking the caller to choose.
pub fn resolve_catalog_package<FChoose>(
    conn: &storage::DbConnection,
    query: &str,
    mut choose_package: FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let matches = search_catalog_packages(conn, query)?;

    if matches.is_empty() {
        anyhow::bail!("no catalog packages matched '{query}'");
    }

    if matches.len() == 1 {
        return Ok(matches.into_iter().next().expect("single match exists"));
    }

    if let Some(exact_index) = matches
        .iter()
        .position(|pkg| pkg.name.eq_ignore_ascii_case(query))
    {
        return Ok(matches
            .into_iter()
            .nth(exact_index)
            .expect("exact match index is valid"));
    }

    let selected = choose_package(query, &matches)?;

    matches
        .into_iter()
        .nth(selected)
        .ok_or_else(|| anyhow::anyhow!("selected package index was out of range"))
}

/// Resolves a package reference, using the chooser only when the reference is name-based and ambiguous.
pub fn resolve_catalog_package_ref<FChoose>(
    conn: &storage::DbConnection,
    package_ref: &PackageRef,
    choose_package: FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    match package_ref {
        PackageRef::ByName(name) => resolve_catalog_package(conn, name, choose_package),
        PackageRef::ById(package_id) => {
            resolve_catalog_package_by_id(conn, &package_id.catalog_id())
        }
    }
}

/// Resolves a package by its exact catalog ID.
pub fn resolve_catalog_package_by_id(
    conn: &storage::DbConnection,
    package_id: &str,
) -> Result<CatalogPackage> {
    storage::get_package_by_id(conn, package_id)?
        .ok_or_else(|| anyhow::anyhow!("no catalog package matched '{package_id}'"))
}
