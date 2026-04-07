pub mod catalog;
pub mod conversion;
pub mod dependency;
pub mod error;
pub mod installer;
pub mod package;
pub mod package_ref;
pub mod validation;
pub mod version;

pub use catalog::{CatalogInstaller, CatalogPackage, RawCatalogInstaller, RawCatalogPackage};
pub use dependency::Dependency;
pub use error::ModelError;
pub use installer::{Architecture, Installer, InstallerType};
pub use package::{Package, PackageKind, PackageSource};
pub use package_ref::{PackageId, PackageRef};
pub use validation::Validate;
pub use version::Version;
