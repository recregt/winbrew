use anyhow::Result;

use crate::database;
use crate::models::CatalogPackage;

pub fn search_packages(query: &str) -> Result<Vec<CatalogPackage>> {
    let conn = database::get_catalog_conn()?;
    database::search(&conn, query)
}

pub fn is_catalog_unavailable(err: &anyhow::Error) -> bool {
    err.downcast_ref::<database::CatalogNotFoundError>()
        .is_some()
}
