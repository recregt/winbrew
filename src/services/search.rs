use anyhow::Result;

use crate::database;
use crate::models::CatalogPackage;

pub fn search_packages(query: &str) -> Result<Vec<CatalogPackage>> {
    let conn = database::get_catalog_conn()?;
    database::search(&conn, query)
}