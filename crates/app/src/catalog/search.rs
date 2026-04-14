//! Catalog lookup and package-resolution helpers.
//!
//! This module turns a user-facing query or package reference into a concrete
//! catalog package. It keeps the resolution rules close to the catalog database
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

use anyhow::{Result, bail};

use crate::database;
use winbrew_models::domains::catalog::CatalogPackage;
use winbrew_models::domains::package::PackageRef;

/// Search the shared catalog connection and return catalog results for the query.
///
/// This is the app-facing search entry point used by callers that do not
/// already have a catalog connection open. Higher layers can map database
/// failures into user-facing errors if they need a narrower error type.
pub(crate) fn search_packages(query: &str) -> Result<Vec<CatalogPackage>> {
    let conn = database::get_catalog_conn()?;
    database::search(&conn, query)
}

/// Resolve a query into a single catalog package.
///
/// The function first runs a catalog search and then applies the disambiguation
/// rules described at the module level. Exact name matches are preferred over
/// interactive selection, and the provided chooser is only consulted when the
/// query still maps to multiple candidates.
fn resolve_catalog_package<FChoose>(
    conn: &database::DbConnection,
    query: &str,
    mut choose_package: FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let mut matches = database::search(conn, query)?;

    if matches.is_empty() {
        bail!("no catalog packages matched '{query}'");
    }

    if matches.len() == 1 {
        return Ok(matches.pop().expect("single match exists"));
    }

    if let Some(exact_index) = matches
        .iter()
        .position(|pkg| pkg.name.eq_ignore_ascii_case(query))
    {
        return Ok(matches.swap_remove(exact_index));
    }

    let selected = choose_package(query, &matches)?;

    if selected >= matches.len() {
        bail!(
            "selected package index {selected} was out of range (0..{})",
            matches.len()
        );
    }

    Ok(matches.swap_remove(selected))
}

/// Resolve a package reference to a single catalog package.
///
/// Name-based references use the same interactive disambiguation rules as a raw
/// query. ID-based references bypass the chooser and resolve by exact catalog
/// ID only, which keeps package references deterministic when the caller has a
/// unique identifier.
pub(crate) fn resolve_catalog_package_ref<FChoose>(
    conn: &database::DbConnection,
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
    conn: &database::DbConnection,
    package_id: &str,
) -> Result<CatalogPackage> {
    database::get_package_by_id(conn, package_id)?
        .ok_or_else(|| anyhow::anyhow!("no catalog package matched '{package_id}'"))
}

#[cfg(test)]
mod tests {
    use super::{resolve_catalog_package_ref, search_packages};
    use crate::database;
    use anyhow::Result;
    use std::fs;
    use std::path::Path;
    use winbrew_models::domains::package::{PackageName, PackageRef};
    use winbrew_testing::{append_catalog_db, init_database, seed_catalog_db, test_root};

    fn prepare_catalog(root: &Path, packages: &[(&str, &str, &str)]) -> Result<()> {
        assert!(!packages.is_empty());

        init_database(root)?;

        let catalog_db_path = root.join("data").join("db").join("catalog.db");
        fs::create_dir_all(
            catalog_db_path
                .parent()
                .expect("catalog database parent directory"),
        )?;

        let (first_name, first_description, first_url) = packages[0];
        seed_catalog_db(
            &catalog_db_path,
            first_name,
            first_description,
            first_url,
            "sha256:11111111",
        )?;

        for (index, &(name, description, url)) in packages.iter().enumerate().skip(1) {
            let hash = format!("sha256:{index:08x}");
            append_catalog_db(&catalog_db_path, name, description, url, &hash)?;
        }

        Ok(())
    }

    #[test]
    fn exact_name_match_bypasses_chooser() -> Result<()> {
        let root = test_root();
        prepare_catalog(
            root.path(),
            &[
                (
                    "Contoso",
                    "Exact match package",
                    "https://example.invalid/contoso.zip",
                ),
                (
                    "Contoso App",
                    "Ambiguous sibling package",
                    "https://example.invalid/contoso-app.zip",
                ),
            ],
        )?;

        let conn = database::get_catalog_conn()?;
        let package = resolve_catalog_package_ref(
            &conn,
            &PackageRef::ByName(PackageName::parse("Contoso")?),
            |_, _| panic!("chooser should not be called for an exact name match"),
        )?;

        assert_eq!(package.name, "Contoso");
        Ok(())
    }

    #[test]
    fn chooser_selection_returns_requested_package() -> Result<()> {
        let root = test_root();
        prepare_catalog(
            root.path(),
            &[
                (
                    "Alpha Tool",
                    "First ambiguous package",
                    "https://example.invalid/alpha.zip",
                ),
                (
                    "Beta Tool",
                    "Second ambiguous package",
                    "https://example.invalid/beta.zip",
                ),
            ],
        )?;

        let conn = database::get_catalog_conn()?;
        let package = resolve_catalog_package_ref(
            &conn,
            &PackageRef::ByName(PackageName::parse("Tool")?),
            |_, matches| {
                Ok(matches
                    .iter()
                    .position(|pkg| pkg.name == "Beta Tool")
                    .expect("beta package should be in the chooser list"))
            },
        )?;

        assert_eq!(package.name, "Beta Tool");
        Ok(())
    }

    #[test]
    fn chooser_selection_rejects_out_of_range_index() -> Result<()> {
        let root = test_root();
        prepare_catalog(
            root.path(),
            &[
                (
                    "Alpha Tool",
                    "First ambiguous package",
                    "https://example.invalid/alpha.zip",
                ),
                (
                    "Beta Tool",
                    "Second ambiguous package",
                    "https://example.invalid/beta.zip",
                ),
            ],
        )?;

        let conn = database::get_catalog_conn()?;
        let err = resolve_catalog_package_ref(
            &conn,
            &PackageRef::ByName(PackageName::parse("Tool")?),
            |_, matches| Ok(matches.len()),
        )
        .expect_err("out-of-range chooser index should fail");

        assert!(err.to_string().contains("out of range"));
        Ok(())
    }

    #[test]
    fn search_packages_returns_results_from_catalog_db() -> Result<()> {
        let root = test_root();
        prepare_catalog(
            root.path(),
            &[(
                "Contoso",
                "Exact match package",
                "https://example.invalid/contoso.zip",
            )],
        )?;

        let packages = search_packages("Contoso")?;

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "Contoso");
        Ok(())
    }
}
