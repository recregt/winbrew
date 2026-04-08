use crate::Package;

#[derive(Debug, Clone)]
pub struct RemovalPlan {
    pub package: Package,
    pub dependents: Vec<String>,
}
