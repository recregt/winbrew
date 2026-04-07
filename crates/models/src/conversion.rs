use crate::catalog::CatalogPackage;
use crate::package::Package;

impl From<&Package> for CatalogPackage {
    fn from(package: &Package) -> Self {
        Self {
            id: package.id.clone().into(),
            name: package.name.clone(),
            version: package.version.clone(),
            source: package.source,
            description: package.description.clone(),
            homepage: package.homepage.clone(),
            license: package.license.clone(),
            publisher: package.publisher.clone(),
        }
    }
}
