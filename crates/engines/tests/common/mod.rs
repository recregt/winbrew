use std::fmt::Debug;

use winbrew_models::catalog::package::CatalogInstaller;
use winbrew_models::install::installer::InstallerType;

pub const PACKAGE_NAME: &str = "Contoso.App";
pub const BASE_URL: &str = "https://example.invalid/";

pub fn url_for(file_name: &str) -> String {
    format!("{BASE_URL}{file_name}")
}

pub fn installer_with_url(
    kind: InstallerType,
    url: &str,
    nested_kind: Option<InstallerType>,
) -> CatalogInstaller {
    let installer = CatalogInstaller::test_builder(PACKAGE_NAME.into(), url).with_kind(kind);

    match nested_kind {
        Some(nested_kind) => installer.with_nested(nested_kind),
        None => installer,
    }
}

pub fn installer(
    kind: InstallerType,
    file_name: &str,
    nested_kind: Option<InstallerType>,
) -> CatalogInstaller {
    let url = url_for(file_name);

    installer_with_url(kind, url.as_str(), nested_kind)
}

pub fn assert_expected<T>(actual: T, expected: T, description: &str)
where
    T: PartialEq + Debug,
{
    assert_eq!(actual, expected, "{description}");
}
