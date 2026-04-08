pub use winbrew_models::{
    Architecture, CatalogInstaller, CatalogPackage, Dependency, InstalledPackage as Package,
    Installer, InstallerType, ModelError, PackageId, PackageKind, PackageName, PackageRef,
    PackageSource, PackageStatus, RawCatalogInstaller, RawCatalogPackage, Validate, Version,
};

pub mod config;
pub mod diagnostics;
pub mod info;
pub mod install;
pub mod package_ref;
pub mod remove;
pub mod report;

pub use diagnostics::DiagnosisResult;
pub use info::InfoReport;
pub use report::{HealthReport, ReportSection, RuntimeReport};

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
