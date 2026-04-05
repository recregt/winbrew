use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

use std::path::PathBuf;

use crate::database;
use crate::engines::{self, EngineKind, PackageEngine};
use crate::models::Package;

#[derive(Debug, Clone)]
pub struct RemovalPlan {
    pub name: String,
    pub kind: String,
    pub install_dir: String,
    pub msix_package_full_name: Option<String>,
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
        name: pkg.name,
        kind: pkg.kind,
        install_dir: pkg.install_dir,
        msix_package_full_name: pkg.msix_package_full_name,
        dependents,
    }
}

fn execute_removal_with_conn(
    plan: &RemovalPlan,
    force: bool,
    conn: &rusqlite::Connection,
) -> Result<()> {
    debug!(package = plan.name.as_str(), force, "starting remove");

    if !force && !plan.dependents.is_empty() {
        bail!(
            "cannot remove '{name}' because it is required by: {}",
            plan.dependents.join(", "),
            name = plan.name
        );
    }

    let install_dir = PathBuf::from(&plan.install_dir);
    let engine_kind = match engines::get_engine_kind(&plan.kind) {
        Ok(engine_kind) => engine_kind,
        Err(_) => bail!("unsupported package type: {}", plan.kind),
    };

    match engine_kind {
        EngineKind::Msix => {
            engine_kind.remove(
                &plan.name,
                &install_dir,
                plan.msix_package_full_name.as_deref(),
            )?;

            if install_dir.exists()
                && let Err(err) = std::fs::remove_dir_all(&install_dir)
            {
                warn!(
                    "failed to remove package directory for {}: {err}",
                    plan.name
                );
            }

            database::delete_package(conn, &plan.name)?;
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

                if let Err(err) = database::delete_package(conn, &plan.name) {
                    let _ = std::fs::rename(&trash_dir, &install_dir);
                    return Err(err).context("failed to remove package from database");
                }

                if let Err(err) = engine_kind.remove(&plan.name, &trash_dir, None) {
                    warn!("failed to completely remove trash for {}: {err}", plan.name);
                }
            } else {
                database::delete_package(conn, &plan.name)?;
            }
        }
    }

    debug!(package = plan.name.as_str(), force, "remove completed");

    Ok(())
}

fn dependency_name(dep: &str) -> &str {
    dep.split_once('@').map(|(name, _)| name).unwrap_or(dep)
}
