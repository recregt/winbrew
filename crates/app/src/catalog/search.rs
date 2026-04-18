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
//!
//! Queries are trimmed before search, must not be empty, and are capped at 256
//! characters so obviously invalid input fails fast.

use anyhow::{Result, bail};

use crate::database;
use crate::models::domains::catalog::CatalogPackage;
use crate::models::domains::package::PackageRef;

const MAX_QUERY_LENGTH: usize = 256;

fn validate_query(query: &str) -> Result<&str> {
    let query = query.trim();

    if query.is_empty() {
        bail!("query cannot be empty or whitespace-only");
    }

    let query_length = query.chars().count();
    if query_length > MAX_QUERY_LENGTH {
        bail!("query too long: {query_length} characters (max {MAX_QUERY_LENGTH})");
    }

    Ok(query)
}

/// Search the shared catalog connection and return catalog results for the query.
///
/// This is the app-facing search entry point used by callers that do not
/// already have a catalog connection open. Higher layers can map database
/// failures into user-facing errors if they need a narrower error type.
pub(crate) fn search_packages(query: &str) -> Result<Vec<CatalogPackage>> {
    let query = validate_query(query)?;
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
    let query = validate_query(query)?;
    let mut matches = database::search(conn, query)?;

    if matches.is_empty() {
        bail!("no catalog packages matched '{query}'");
    }

    if matches.len() == 1 {
        debug_assert_eq!(
            matches.len(),
            1,
            "single-match branch must contain exactly one package"
        );
        return matches.pop().ok_or_else(|| {
            anyhow::anyhow!("internal error: expected single match but vector was empty")
        });
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
    use super::{MAX_QUERY_LENGTH, resolve_catalog_package_ref, search_packages};
    use crate::database;
    use crate::models::domains::catalog::CatalogPackage;
    use crate::models::domains::package::{PackageName, PackageRef};
    use anyhow::Result;
    use std::fs;
    use tempfile::TempDir;
    use winbrew_testing::{append_catalog_db, init_database, seed_catalog_db, test_root};

    struct TestCatalog {
        _root: TempDir,
    }

    impl TestCatalog {
        fn with_packages(packages: &[(&str, &str, &str)]) -> Result<Self> {
            assert!(!packages.is_empty());

            let root = test_root();
            init_database(root.path())?;

            let catalog_db_path = root.path().join("data").join("db").join("catalog.db");
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

            Ok(Self { _root: root })
        }

        fn conn(&self) -> Result<database::DbConnection> {
            database::get_catalog_conn()
        }

        fn resolve_by_name(&self, name: &str) -> Result<CatalogPackage> {
            self.resolve_by_name_with_chooser(name, |_, _| {
                panic!("chooser should not be called for an exact name match")
            })
        }

        fn resolve_by_name_with_chooser<F>(&self, name: &str, chooser: F) -> Result<CatalogPackage>
        where
            F: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
        {
            let conn = self.conn()?;
            resolve_catalog_package_ref(
                &conn,
                &PackageRef::ByName(PackageName::parse(name)?),
                chooser,
            )
        }

        fn resolve_ref(&self, package_ref: &str) -> Result<CatalogPackage> {
            let conn = self.conn()?;
            let package_ref = PackageRef::parse(package_ref)?;

            resolve_catalog_package_ref(&conn, &package_ref, |_, _| {
                panic!("chooser should not be called for exact id resolution")
            })
        }

        fn search(&self, query: &str) -> Result<Vec<CatalogPackage>> {
            search_packages(query)
        }
    }

    #[test]
    fn exact_name_match_bypasses_chooser() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[
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
        ])?;

        let package = catalog.resolve_by_name("Contoso")?;

        assert_eq!(package.name, "Contoso");
        Ok(())
    }

    #[test]
    fn case_insensitive_exact_name_match_is_preferred() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[(
            "Contoso",
            "Exact match package",
            "https://example.invalid/contoso.zip",
        )])?;

        let package = catalog.resolve_by_name("contoso")?;

        assert_eq!(package.name, "Contoso");
        Ok(())
    }

    #[test]
    fn chooser_selection_returns_requested_package() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[
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
        ])?;

        let package = catalog.resolve_by_name_with_chooser("Tool", |query, matches| {
            assert_eq!(query, "Tool");
            assert_eq!(matches.len(), 2);
            Ok(matches
                .iter()
                .position(|pkg| pkg.name == "Beta Tool")
                .expect("beta package should be in the chooser list"))
        })?;

        assert_eq!(package.name, "Beta Tool");
        Ok(())
    }

    #[test]
    fn chooser_selection_rejects_out_of_range_index() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[
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
        ])?;

        let err = catalog
            .resolve_by_name_with_chooser("Tool", |_, matches| Ok(matches.len()))
            .expect_err("out-of-range chooser index should fail");

        assert!(err.to_string().contains("out of range"));
        Ok(())
    }

    #[test]
    fn search_packages_returns_results_from_catalog_db() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[(
            "Contoso",
            "Exact match package",
            "https://example.invalid/contoso.zip",
        )])?;

        let packages = catalog.search("Contoso")?;

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "Contoso");
        Ok(())
    }

    #[test]
    fn whitespace_only_query_is_rejected() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[(
            "Contoso",
            "Exact match package",
            "https://example.invalid/contoso.zip",
        )])?;

        let err = catalog
            .search("   ")
            .expect_err("blank query should be rejected");

        assert!(err.to_string().contains("query cannot be empty"));
        Ok(())
    }

    #[test]
    fn trimmed_queries_still_search_successfully() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[(
            "Contoso",
            "Exact match package",
            "https://example.invalid/contoso.zip",
        )])?;

        let packages = catalog.search("  Contoso  ")?;

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "Contoso");
        Ok(())
    }

    #[test]
    fn very_long_query_is_rejected() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[(
            "Contoso",
            "Exact match package",
            "https://example.invalid/contoso.zip",
        )])?;

        let query = "a".repeat(MAX_QUERY_LENGTH + 1);
        let err = catalog
            .search(&query)
            .expect_err("oversized query should be rejected");

        assert!(err.to_string().contains("query too long"));
        Ok(())
    }

    #[test]
    fn id_resolution_is_deterministic() -> Result<()> {
        let catalog = TestCatalog::with_packages(&[(
            "Contoso",
            "Exact match package",
            "https://example.invalid/contoso.zip",
        )])?;

        let first = catalog.resolve_ref("@winget/Contoso")?;
        let second = catalog.resolve_ref("@winget/Contoso")?;

        assert_eq!(first.id, second.id);
        assert_eq!(first.name, "Contoso");
        Ok(())
    }
}
