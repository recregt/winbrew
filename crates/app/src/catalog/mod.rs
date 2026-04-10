//! Catalog search and installer selection helpers.

pub mod error;
pub mod search;
pub mod select;

pub use error::{InstallerSelectionError, SearchError, SearchResult};
pub use search::{
    resolve_catalog_package, resolve_catalog_package_by_id, resolve_catalog_package_ref,
    search_catalog_packages, search_packages,
};
pub use select::select_installer;
