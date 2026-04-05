use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

use std::path::PathBuf;

use crate::database;
use crate::engines::{self, EngineKind, PackageEngine};
use crate::models::Package;

#[derive(Debug, Clone)]
pub struct RemovalPlan {
    pub package: Package,
    pub dependents: Vec<String>,
}

pub fn find_dependents(name: &str, conn: &rusqlite::Connection) -> Result<Vec<String>> {
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

pub fn plan_removal(name: &str) -> Result<RemovalPlan> {
    let conn = database::get_conn()?;
    let pkg =
        database::get_package(&conn, name)?.ok_or_else(|| database::PackageNotFoundError {
            name: name.to_string(),
        })?;
    let dependents = find_dependents(name, &conn)?;

    Ok(removal_plan(pkg, dependents))
}

pub fn execute_removal(plan: &RemovalPlan, force: bool) -> Result<()> {
    let conn = database::get_conn()?;

    execute_removal_with_conn(plan, force, &conn)
}

pub fn remove(name: &str, force: bool) -> Result<()> {
    let plan = plan_removal(name)?;

    execute_removal(&plan, force)
}

fn removal_plan(pkg: Package, dependents: Vec<String>) -> RemovalPlan {
    RemovalPlan {
        package: pkg,
        dependents,
    }
}

fn execute_removal_with_conn(
    plan: &RemovalPlan,
    force: bool,
    conn: &rusqlite::Connection,
) -> Result<()> {
    debug!(
        package = plan.package.name.as_str(),
        force, "starting remove"
    );

    if !force && !plan.dependents.is_empty() {
        bail!(
            "cannot remove '{name}' because it is required by: {}",
            plan.dependents.join(", "),
            name = plan.package.name
        );
    }

    let install_dir = PathBuf::from(&plan.package.install_dir);
    let engine_kind = match engines::get_engine_kind(&plan.package.kind) {
        Ok(engine_kind) => engine_kind,
        Err(_) => bail!("unsupported package type: {}", plan.package.kind),
    };

    match engine_kind {
        EngineKind::Msix => {
            engine_kind.remove(&plan.package)?;

            if install_dir.exists()
                && let Err(err) = std::fs::remove_dir_all(&install_dir)
            {
                warn!(
                    "failed to remove package directory for {}: {err}",
                    plan.package.name
                );
            }

            database::delete_package(conn, &plan.package.name)?;
        }
        EngineKind::Zip | EngineKind::Portable => {
            if install_dir.exists() {
                let trash_dir = install_dir.with_extension("trash");

                if trash_dir.exists() {
                    std::fs::remove_dir_all(&trash_dir)
                        .context("failed to clean up old trash directory")?;
                }

                std::fs::rename(&install_dir, &trash_dir)
                    .context("failed to stage package for removal")?;

                let trash_package = Package {
                    install_dir: trash_dir.to_string_lossy().into_owned(),
                    ..plan.package.clone()
                };

                if let Err(err) = database::delete_package(conn, &plan.package.name) {
                    let _ = std::fs::rename(&trash_dir, &install_dir);
                    return Err(err).context("failed to remove package from database");
                }

                if let Err(err) = engine_kind.remove(&trash_package) {
                    warn!(
                        "failed to completely remove trash for {}: {err}",
                        plan.package.name
                    );
                }
            } else {
                database::delete_package(conn, &plan.package.name)?;
            }
        }
    }

    debug!(
        package = plan.package.name.as_str(),
        force, "remove completed"
    );

    Ok(())
}

fn dependency_name(dep: &str) -> &str {
    dep.split_once('@').map(|(name, _)| name).unwrap_or(dep)
}

#[cfg(test)]
mod tests {
    use super::removal_plan;
    use crate::models::{Package, PackageStatus};

    fn package(
        name: &str,
        kind: &str,
        install_dir: &str,
        msix_package_full_name: Option<&str>,
    ) -> Package {
        Package {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: kind.to_string(),
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
                "msix",
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
