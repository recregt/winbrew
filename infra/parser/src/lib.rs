mod error;
mod metadata;
mod parser;
mod pipeline;
pub(crate) mod raw;
mod sqlite;
mod winget;

pub use error::ParserError;
pub use metadata::CatalogMetadata;
pub use parser::{ParsedPackage, parse_package, parse_packages, parse_packages_json};
pub use pipeline::{RunConfig, run};
pub use raw::{RawFetchedInstaller, RawFetchedPackage, ScoopStreamEnvelope};
