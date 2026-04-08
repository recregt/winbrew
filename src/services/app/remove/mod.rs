mod execution;
mod plan;

use thiserror::Error;

use winbrew_models::InstallerType;

pub use execution::execute_removal;
pub use plan::{find_dependents, plan_removal};
pub use winbrew_models::RemovalPlan;

#[derive(Debug, Error)]
pub enum RemovalError {
    #[error("cannot remove '{name}' because it is required by: {dependents}")]
    DependentPackagesBlocked { name: String, dependents: String },

    #[error("unsupported package type: {kind}")]
    UnsupportedPackageType { kind: InstallerType },

    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, RemovalError>;

pub fn remove(name: &str, force: bool) -> Result<()> {
    let plan = plan_removal(name)?;

    execute_removal(&plan, force)
}
