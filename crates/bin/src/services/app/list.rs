use anyhow::Result;

use crate::models::{Package, PackageQuery};
use crate::services::shared::storage;

pub fn list_packages(query: Option<&str>) -> Result<Vec<Package>> {
    let conn = storage::get_conn()?;
    let packages = storage::list_packages(&conn)?;

    Ok(match query {
        Some(query) if !query.trim().is_empty() => {
            let query = PackageQuery {
                terms: query.split_whitespace().map(ToOwned::to_owned).collect(),
                version: None,
            };
            filter_packages(packages, &query.text())
        }
        _ => packages,
    })
}

fn filter_packages(packages: Vec<Package>, query: &str) -> Vec<Package> {
    let normalized_query = normalize(query);

    packages
        .into_iter()
        .filter(|pkg| package_matches(pkg, &normalized_query))
        .collect()
}

fn package_matches(pkg: &Package, query: &str) -> bool {
    let haystack = [
        pkg.name.as_str(),
        pkg.version.as_str(),
        pkg.kind.as_str(),
        pkg.install_dir.as_str(),
    ]
    .into_iter()
    .map(normalize)
    .collect::<Vec<_>>()
    .join(" ");

    query.split_whitespace().all(|term| haystack.contains(term))
}

fn normalize(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{filter_packages, normalize};
    use crate::models::{InstallerType, Package, PackageStatus};

    fn package(name: &str, version: &str, kind: InstallerType, install_dir: &str) -> Package {
        Package {
            name: name.to_string(),
            version: version.to_string(),
            kind,
            install_dir: install_dir.to_string(),
            msix_package_full_name: None,
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn normalize_collapses_punctuation_and_whitespace() {
        assert_eq!(
            normalize("Contoso.App v1.0\t(Stable)"),
            "contoso app v1 0 stable"
        );
    }

    #[test]
    fn filter_packages_matches_terms_across_display_fields() {
        let packages = vec![
            package(
                "Contoso App",
                "1.2.3",
                InstallerType::Msix,
                r"C:\Packages\Contoso.App",
            ),
            package(
                "Other Tool",
                "2.0.0",
                InstallerType::Portable,
                r"C:\Tools\Other",
            ),
        ];

        let matches = filter_packages(packages, "contoso 1.2 msix");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Contoso App");
    }
}
