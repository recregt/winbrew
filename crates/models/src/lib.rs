pub mod catalog;
pub mod catalog_metadata;
pub mod config;
pub mod conversion;
pub mod dependency;
pub mod diagnostics;
pub mod engine;
pub mod error;
pub mod hash;
pub mod identifiers;
pub mod info;
pub mod install;
pub mod installed;
pub mod installer;
pub mod msi_inventory;
pub mod package;
pub mod package_ref;
pub mod query;
pub mod raw;
pub mod remove;
pub mod report;
pub mod validation;
pub mod version;

pub use catalog::{CatalogInstaller, CatalogPackage, RawCatalogInstaller, RawCatalogPackage};
pub use catalog_metadata::CatalogMetadata;
pub use config::{ConfigSection, ConfigValue, ConfigValueSource};
pub use dependency::Dependency;
pub use diagnostics::{DiagnosisResult, DiagnosisSeverity};
pub use engine::{EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope};
pub use error::ModelError;
pub use hash::HashAlgorithm;
pub use identifiers::{BucketName, CatalogId, PackageName};
pub use info::InfoReport;
pub use install::{InstallFailureClass, InstallOutcome, InstallResult};
pub use installed::{InstalledPackage, InstalledPackage as Package, PackageStatus};
pub use installer::{Architecture, Installer, InstallerType};
pub use msi_inventory::{
    MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
    MsiRegistryRecord, MsiShortcutRecord,
};
pub use package::{PackageKind, PackageSource};
pub use package_ref::{PackageId, PackageRef};
pub use query::PackageQuery;
pub use raw::{RawFetchedInstaller, RawFetchedPackage, ScoopStreamEnvelope};
pub use remove::RemovalPlan;
pub use report::{
    HealthReport, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind, ReportSection,
    RuntimeReport,
};
pub use validation::Validate;
pub use version::Version;
