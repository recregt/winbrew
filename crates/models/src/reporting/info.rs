use super::report::RuntimeReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoReport {
    pub version: String,
    pub runtime: RuntimeReport,
}
