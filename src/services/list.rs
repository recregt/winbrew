use anyhow::Result;

use crate::database;
use crate::models::Package;
use crate::models::PackageQuery;

pub fn list_packages(query: Option<&str>) -> Result<Vec<Package>> {
    let conn = database::get_conn()?;
    let packages = database::list_packages(&conn)?;

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
    let haystack = [&pkg.name, &pkg.version, &pkg.kind, &pkg.install_dir]
        .into_iter()
        .map(|value| normalize(value))
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
