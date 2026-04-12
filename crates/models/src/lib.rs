pub mod catalog;
pub mod catalog_metadata;
pub mod config;
pub mod conversion;
pub mod dependency;
pub mod engine;
pub mod error;
pub mod hash;
pub mod identifiers;
pub mod install;
pub mod installed;
pub mod installer;
pub mod msi_inventory;
pub mod package;
pub mod package_ref;
pub mod query;
pub mod raw;
pub mod remove;
pub mod reporting;
pub mod shared;
pub mod validation;
pub mod version;

/// Grouped namespace for the major model families.
pub mod domains {
    pub mod shared {
        pub use crate::shared::{BucketName, CatalogId, PackageName};
        pub use crate::shared::{
            ConfigSection, ConfigValue, ConfigValueSource, HashAlgorithm, ModelError, Validate,
            Version,
        };
    }

    pub mod package {
        pub use crate::PackageName;
        pub use crate::package::{
            Dependency, Package, PackageId, PackageKind, PackageQuery, PackageRef, PackageSource,
        };
    }

    pub mod catalog {
        pub use crate::{
            CatalogInstaller, CatalogMetadata, CatalogPackage, RawCatalogInstaller,
            RawCatalogPackage,
        };
    }

    pub mod installed {
        pub use crate::{InstalledPackage, PackageStatus};
    }

    pub mod install {
        pub use crate::{
            Architecture, EngineInstallReceipt, EngineKind, EngineMetadata, InstallFailureClass,
            InstallOutcome, InstallResult, InstallScope, Installer, InstallerType, RemovalPlan,
        };
    }

    pub mod reporting {
        pub use crate::reporting::{
            DiagnosisResult, DiagnosisSeverity, HealthReport, InfoReport, RecoveryActionGroup,
            RecoveryFinding, RecoveryIssueKind, ReportSection, RuntimeReport,
        };
    }

    pub mod inventory {
        pub use crate::{
            MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
            MsiRegistryRecord, MsiShortcutRecord,
        };
    }
}

pub use catalog::{CatalogInstaller, CatalogPackage, RawCatalogInstaller, RawCatalogPackage};
pub use catalog_metadata::CatalogMetadata;
pub use config::{ConfigSection, ConfigValue, ConfigValueSource};
pub use dependency::Dependency;
pub use engine::{EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope};
pub use error::ModelError;
pub use hash::HashAlgorithm;
pub use identifiers::{BucketName, CatalogId, PackageName};
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
pub use reporting::{
    DiagnosisResult, DiagnosisSeverity, HealthReport, InfoReport, RecoveryActionGroup,
    RecoveryFinding, RecoveryIssueKind, ReportSection, RuntimeReport,
};
pub use validation::Validate;
pub use version::Version;
