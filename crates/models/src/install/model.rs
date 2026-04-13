use crate::shared::HashAlgorithm;

/// Failure buckets used by install orchestration and user-facing errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFailureClass {
    /// Failure while checking preconditions.
    Preflight,
    /// Failure while verifying a downloaded artifact.
    Verification,
    /// Failure while transitioning install state in storage.
    StateTransition,
    /// Failure due to cancellation.
    Cancelled,
    /// Failure caused by a runtime engine error.
    Runtime,
}

/// The successful result of an install flow.
#[derive(Debug, Clone)]
pub struct InstallResult {
    /// Package name reported by the install flow.
    pub name: String,
    /// Package version reported by the install flow.
    pub version: String,
    /// Final install directory reported by the engine after installation.
    pub install_dir: String,
}

/// Full outcome of an install attempt.
#[derive(Debug, Clone)]
pub struct InstallOutcome {
    /// Successful install result.
    pub result: InstallResult,
    /// Legacy checksum algorithms encountered during verification.
    pub legacy_checksum_algorithms: Vec<HashAlgorithm>,
}
