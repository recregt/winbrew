use anyhow::Result;

use crate::storage;
use winbrew_models::{CatalogPackage, PackageRef};

#[derive(Debug)]
pub enum SearchError {
    CatalogUnavailable,
    Unexpected(anyhow::Error),
}

pub type SearchResult<T> = std::result::Result<T, SearchError>;

impl From<anyhow::Error> for SearchError {
    fn from(value: anyhow::Error) -> Self {
        Self::Unexpected(value)
    }
}

pub fn search_catalog_packages(
    conn: &storage::DbConnection,
    query: &str,
) -> Result<Vec<CatalogPackage>> {
    storage::search(conn, query)
}

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
        Err(err)
            if err
                .downcast_ref::<storage::CatalogNotFoundError>()
                .is_some() =>
        {
            Err(SearchError::CatalogUnavailable)
        }
        Err(err) => Err(SearchError::Unexpected(err)),
    }
}

pub fn resolve_catalog_package<FChoose>(
    conn: &storage::DbConnection,
    query: &str,
    choose_package: &mut FChoose,
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
        return Ok(matches.into_iter().nth(exact_index).unwrap());
    }

    let selected = choose_package(query, &matches)?;

    matches
        .into_iter()
        .nth(selected)
        .ok_or_else(|| anyhow::anyhow!("selected package index was out of range"))
}

pub fn resolve_catalog_package_ref<FChoose>(
    conn: &storage::DbConnection,
    package_ref: &PackageRef,
    choose_package: &mut FChoose,
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

pub fn resolve_catalog_package_by_id(
    conn: &storage::DbConnection,
    package_id: &str,
) -> Result<CatalogPackage> {
    storage::get_package_by_id(conn, package_id)?
        .ok_or_else(|| anyhow::anyhow!("no catalog package matched '{package_id}'"))
}
