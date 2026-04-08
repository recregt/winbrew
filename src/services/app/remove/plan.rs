use crate::models::Package;
use crate::models::remove::RemovalPlan;
use crate::services::shared::storage;

use super::Result;

pub fn find_dependents(name: &str, conn: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut dependents = storage::list_packages(conn)?
        .into_iter()
        .filter(|pkg| {
            pkg.name != name
                && pkg
                    .dependencies
                    .iter()
                    .any(|dep| dependency_name(dep).eq_ignore_ascii_case(name))
        })
        .map(|pkg| pkg.name)
        .collect::<Vec<_>>();

    dependents.sort_unstable();
    dependents.dedup();

    Ok(dependents)
}

pub fn plan_removal(name: &str) -> Result<RemovalPlan> {
    let conn = storage::get_conn()?;
    let pkg = storage::get_package(&conn, name)?.ok_or_else(|| {
        anyhow::Error::new(storage::PackageNotFoundError {
            name: name.to_string(),
        })
    })?;
    let dependents = find_dependents(name, &conn)?;

    Ok(removal_plan(pkg, dependents))
}

fn removal_plan(pkg: Package, dependents: Vec<String>) -> RemovalPlan {
    RemovalPlan {
        package: pkg,
        dependents,
    }
}

fn dependency_name(dep: &str) -> &str {
    dep.split_once('@').map_or(dep, |(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::removal_plan;
    use crate::models::{InstallerType, Package, PackageStatus};

    fn package(
        name: &str,
        kind: InstallerType,
        install_dir: &str,
        msix_package_full_name: Option<&str>,
    ) -> Package {
        Package {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind,
            install_dir: install_dir.to_string(),
            msix_package_full_name: msix_package_full_name.map(ToOwned::to_owned),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn removal_plan_preserves_msix_full_name() {
        let plan = removal_plan(
            package(
                "Contoso.App",
                InstallerType::Msix,
                r"C:\Packages\Contoso.App",
                Some("Contoso.App_1.0.0_x64__8wekyb3d8bbwe"),
            ),
            vec!["Contoso.Consumer".to_string()],
        );

        assert_eq!(plan.package.name, "Contoso.App");
        assert_eq!(
            plan.package.msix_package_full_name.as_deref(),
            Some("Contoso.App_1.0.0_x64__8wekyb3d8bbwe")
        );
        assert_eq!(plan.dependents, vec!["Contoso.Consumer".to_string()]);
    }
}
