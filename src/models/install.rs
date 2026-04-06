use crate::core::hash::HashAlgorithm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFailureClass {
    Preflight,
    Verification,
    StateTransition,
    Cancelled,
    Runtime,
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub name: String,
    pub version: String,
    pub install_dir: String,
}

#[derive(Debug, Clone)]
pub struct InstallOutcome {
    pub result: InstallResult,
    pub legacy_checksum_algorithms: Vec<HashAlgorithm>,
}
