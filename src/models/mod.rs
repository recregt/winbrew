use serde::{Deserialize, Serialize};

pub mod config;
pub mod diagnostics;
pub mod install;
pub mod remove;
pub mod report;

pub use diagnostics::DiagnosisResult;
pub use report::{HealthReport, ReportSection, RuntimeReport};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PackageStatus {
    Installing,
    Ok,
    Updating,
    Failed,
}

impl PackageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Installing => "installing",
            Self::Ok => "ok",
            Self::Updating => "updating",
            Self::Failed => "failed",
        }
    }

    pub fn parse(status: &str) -> Self {
        match status {
            "ok" => Self::Ok,
            "updating" => Self::Updating,
            "failed" => Self::Failed,
            _ => Self::Installing,
        }
    }
}

impl std::fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub kind: String,
    pub install_dir: String,
    pub msix_package_full_name: Option<String>,
    pub dependencies: Vec<String>,
    pub status: PackageStatus,
    pub installed_at: String,
}

#[derive(Debug, Clone)]
pub struct PackageQuery {
    pub terms: Vec<String>,
    pub version: Option<String>,
}

impl PackageQuery {
    pub fn text(&self) -> String {
        self.terms.join(" ")
    }
}

#[derive(Debug, Clone)]
pub struct CatalogPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CatalogInstaller {
    pub package_id: String,
    pub url: String,
    pub hash: String,
    pub arch: String,
    pub kind: String,
}
