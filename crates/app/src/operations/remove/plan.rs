//! Removal planning and dependency analysis.
//!
//! The planning phase reads the package database, resolves the package to
//! remove, and collects any installed packages that still depend on it. The
//! resulting [`RemovalPlan`] is a snapshot of what the execution phase should
//! remove, not a live view of the database.
//!
//! This split matters because the CLI wants to inspect the plan before it
//! mutates anything. It can display dependents, ask for confirmation, and only
//! then hand the plan to the execution layer.

use crate::models::domains::install::RemovalPlan;
use crate::models::domains::installed::InstalledPackage;
use crate::storage::database;

use super::Result;

/// Find installed packages that depend on the named package.
///
/// Dependency entries may include a version suffix in the form `name@version`.
/// Only the package name is used for matching so the dependency check remains
/// stable even if the dependency recorded a specific version.
pub fn find_dependents(name: &str, conn: &database::DbConnection) -> Result<Vec<String>> {
    let mut dependents = database::list_packages(conn)?
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

/// Build a removal plan for the named package.
///
/// The function loads the package record, gathers current dependents, and then
/// returns a single immutable plan that can be inspected by the caller before
/// removal starts. If the package does not exist, the database error is
/// preserved so the CLI can report that the target package was not found.
pub fn plan_removal(name: &str) -> Result<RemovalPlan> {
    let conn = database::get_conn()?;
    let pkg = database::get_package(&conn, name)?.ok_or_else(|| {
        anyhow::Error::new(database::PackageNotFoundError {
            name: name.to_string(),
        })
    })?;
    let dependents = find_dependents(name, &conn)?;

    Ok(removal_plan(pkg, dependents))
}

/// Construct a removal plan from an already loaded package and dependent list.
///
/// This helper keeps the public planning API focused on database access while
/// still making the plan shape easy to test in isolation.
fn removal_plan(pkg: InstalledPackage, dependents: Vec<String>) -> RemovalPlan {
    RemovalPlan {
        package: pkg,
        dependents,
    }
}

/// Extract the dependency package name from a stored dependency string.
///
/// Dependency records may carry a version suffix after `@`; only the package
/// name participates in removal dependency matching.
fn dependency_name(dep: &str) -> &str {
    dep.split_once('@').map_or(dep, |(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::removal_plan;
    use crate::models::domains::install::EngineMetadata;
    use crate::models::domains::install::InstallScope;
    use crate::models::domains::install::InstallerType;
    use crate::models::domains::installed::{InstalledPackage, PackageStatus};

    fn package(
        name: &str,
        kind: InstallerType,
        install_dir: &str,
        engine_metadata: Option<EngineMetadata>,
    ) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind,
            deployment_kind: kind.deployment_kind(),
            engine_kind: kind.into(),
            engine_metadata,
            install_dir: install_dir.to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn removal_plan_preserves_engine_metadata() {
        let plan = removal_plan(
            package(
                "Contoso.App",
                InstallerType::Msix,
                r"C:\Packages\Contoso.App",
                Some(EngineMetadata::msix(
                    "Contoso.App_1.0.0_x64__8wekyb3d8bbwe",
                    InstallScope::Installed,
                )),
            ),
            vec!["Contoso.Consumer".to_string()],
        );

        assert_eq!(plan.package.name, "Contoso.App");
        assert_eq!(
            plan.package.engine_metadata,
            Some(EngineMetadata::msix(
                "Contoso.App_1.0.0_x64__8wekyb3d8bbwe",
                InstallScope::Installed,
            ))
        );
        assert_eq!(plan.dependents, vec!["Contoso.Consumer".to_string()]);
    }
}
