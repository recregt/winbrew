pub mod search;
pub mod select;

pub use search::{
    SearchError, SearchResult, resolve_catalog_package, search_catalog_packages, search_packages,
};
pub use select::{InstallerSelectionError, select_installer};
