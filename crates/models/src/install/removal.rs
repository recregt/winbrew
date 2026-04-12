use crate::InstalledPackage;

#[derive(Debug, Clone)]
pub struct RemovalPlan {
    pub package: InstalledPackage,
    pub dependents: Vec<String>,
}
