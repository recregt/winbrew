#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosisSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosisResult {
    pub error_code: String,
    pub description: String,
    pub severity: DiagnosisSeverity,
}
