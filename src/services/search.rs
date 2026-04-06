use crate::database;
use crate::models::CatalogPackage;

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

pub fn search_packages(query: &str) -> SearchResult<Vec<CatalogPackage>> {
    let conn = database::get_catalog_conn().map_err(SearchError::from)?;

    match database::search(&conn, query) {
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
