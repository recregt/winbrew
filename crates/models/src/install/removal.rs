use crate::install::InstalledPackage;

/// The removal plan derived from installed-package state and dependents.
#[derive(Debug, Clone)]
pub struct RemovalPlan {
    /// The package selected for removal.
    pub package: InstalledPackage,
    /// Package names that depend on the target package.
    pub dependents: Vec<String>,
}
