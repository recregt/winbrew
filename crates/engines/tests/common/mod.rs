use winbrew_models::catalog::package::CatalogInstaller;
use winbrew_models::install::installer::InstallerType;

pub const PACKAGE_NAME: &str = "Contoso.App";
pub const BASE_URL: &str = "https://example.invalid/";

pub fn url_for(file_name: &str) -> String {
    format!("{BASE_URL}{file_name}")
}

pub fn installer(
    kind: InstallerType,
    file_name: &str,
    nested_kind: Option<InstallerType>,
) -> CatalogInstaller {
    let url = url_for(file_name);
    let installer =
        CatalogInstaller::test_builder(PACKAGE_NAME.into(), url.as_str()).with_kind(kind);

    match nested_kind {
        Some(nested_kind) => installer.with_nested(nested_kind),
        None => installer,
    }
}
