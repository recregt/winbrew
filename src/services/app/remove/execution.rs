use anyhow::Context;
use tracing::{debug, warn};

use std::path::PathBuf;

use crate::engines::{self, EngineKind, PackageEngine};
use crate::models::Package;
use crate::services::shared::storage;

use super::{RemovalError, RemovalPlan, Result};

pub fn execute_removal(plan: &RemovalPlan, force: bool) -> Result<()> {
    let conn = storage::get_conn()?;

    execute_removal_with_conn(plan, force, &conn)
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
        return Err(RemovalError::DependentPackagesBlocked {
            name: plan.package.name.clone(),
            dependents: plan.dependents.join(", "),
        });
    }

    let install_dir = PathBuf::from(&plan.package.install_dir);
    let engine_kind = match engines::get_engine_kind(plan.package.kind) {
        Ok(engine_kind) => engine_kind,
        Err(_) => {
            return Err(RemovalError::UnsupportedPackageType {
                kind: plan.package.kind,
            });
        }
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

            storage::delete_package(conn, &plan.package.name)?;
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

                if let Err(err) = storage::delete_package(conn, &plan.package.name) {
                    let _ = std::fs::rename(&trash_dir, &install_dir);
                    return Err(RemovalError::Unexpected(err));
                }

                if let Err(err) = engine_kind.remove(&trash_package) {
                    warn!(
                        "failed to completely remove trash for {}: {err}",
                        plan.package.name
                    );
                }
            } else {
                storage::delete_package(conn, &plan.package.name)?;
            }
        }
    }

    debug!(
        package = plan.package.name.as_str(),
        force, "remove completed"
    );

    Ok(())
}
