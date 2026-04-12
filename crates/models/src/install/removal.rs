use crate::install::InstalledPackage;

#[derive(Debug, Clone)]
pub struct RemovalPlan {
    pub package: InstalledPackage,
    pub dependents: Vec<String>,
}
