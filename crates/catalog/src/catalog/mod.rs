pub mod search;
pub mod select;

pub use search::{
    SearchError, SearchResult, resolve_catalog_package, resolve_catalog_package_by_id,
    resolve_catalog_package_ref, search_catalog_packages, search_packages,
};
pub use select::{InstallerSelectionError, select_installer};
