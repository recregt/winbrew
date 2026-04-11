//! Catalog lookup and package-resolution helpers.
//!
//! This module turns a user-facing query or package reference into a concrete
//! catalog package. It keeps the resolution rules close to the catalog storage
//! layer so callers can depend on a single, consistent interpretation of the
//! catalog database.
//!
//! Resolution follows a strict order:
//!
//! - exact package ID lookups are resolved directly
//! - a single name match is returned immediately
//! - exact name matches win when multiple packages share the same query
//! - ambiguous name queries delegate the final choice to the caller
//!
//! The goal is to keep search semantics predictable for the CLI while still
//! allowing interactive disambiguation when multiple packages match a name.

use anyhow::Result;

use crate::storage;
use winbrew_models::{CatalogPackage, PackageRef};

/// Search the catalog using an already-open catalog database connection.
///
/// This helper exists for callers that already resolved the catalog connection
/// and only need query-level failure semantics. It does not open or validate
/// the catalog database on its own.
fn search_catalog_packages(
    conn: &storage::DbConnection,
    query: &str,
) -> Result<Vec<CatalogPackage>> {
    storage::search(conn, query)
}

/// Search the shared catalog connection and return catalog results for the query.
///
/// This is the app-facing search entry point used by callers that do not
/// already have a catalog connection open. Higher layers can map storage
/// failures into user-facing errors if they need a narrower error type.
pub(crate) fn search_packages(query: &str) -> Result<Vec<CatalogPackage>> {
    let conn = storage::get_catalog_conn()?;
    storage::search(&conn, query)
}

/// Resolve a query into a single catalog package.
///
/// The function first runs a catalog search and then applies the disambiguation
/// rules described at the module level. Exact name matches are preferred over
/// interactive selection, and the provided chooser is only consulted when the
/// query still maps to multiple candidates.
fn resolve_catalog_package<FChoose>(
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

/// Resolve a package reference to a single catalog package.
///
/// Name-based references use the same interactive disambiguation rules as a raw
/// query. ID-based references bypass the chooser and resolve by exact catalog
/// ID only, which keeps package references deterministic when the caller has a
/// unique identifier.
pub(crate) fn resolve_catalog_package_ref<FChoose>(
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

/// Resolve a package by exact catalog ID.
///
/// The catalog ID path never asks the caller to choose between matches because
/// a package ID is expected to identify exactly one record.
fn resolve_catalog_package_by_id(
    conn: &storage::DbConnection,
    package_id: &str,
) -> Result<CatalogPackage> {
    storage::get_package_by_id(conn, package_id)?
        .ok_or_else(|| anyhow::anyhow!("no catalog package matched '{package_id}'"))
}
