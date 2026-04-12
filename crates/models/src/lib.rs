pub mod catalog;
pub mod config;
pub mod error;
pub mod hash;
pub mod install;
pub mod msi_inventory;
pub mod package;
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
        pub use crate::package::{
            Dependency, Package, PackageId, PackageKind, PackageQuery, PackageRef, PackageSource,
        };
        pub use crate::shared::PackageName;
    }

    pub mod catalog {
        pub use crate::catalog::{
            CatalogInstaller, CatalogMetadata, CatalogPackage, RawCatalogInstaller,
            RawCatalogPackage,
        };
    }

    pub mod installed {
        pub use crate::install::{InstalledPackage, PackageStatus};
    }

    pub mod install {
        pub use crate::install::{
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
        pub use crate::msi_inventory::{
            MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
            MsiRegistryRecord, MsiShortcutRecord,
        };
    }
}

pub use catalog::{
    CatalogInstaller, CatalogMetadata, CatalogPackage, RawCatalogInstaller, RawCatalogPackage,
};
pub use config::{ConfigSection, ConfigValue, ConfigValueSource};
pub use error::ModelError;
pub use hash::HashAlgorithm;
pub use install::{
    Architecture, EngineInstallReceipt, EngineKind, EngineMetadata, InstallFailureClass,
    InstallOutcome, InstallResult, InstallScope, InstalledPackage, Installer, InstallerType,
    PackageStatus, RemovalPlan,
};
pub use msi_inventory::{
    MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
    MsiRegistryRecord, MsiShortcutRecord,
};
pub use package::{Dependency, PackageId, PackageKind, PackageQuery, PackageRef, PackageSource};
pub use reporting::{
    DiagnosisResult, DiagnosisSeverity, HealthReport, InfoReport, RecoveryActionGroup,
    RecoveryFinding, RecoveryIssueKind, ReportSection, RuntimeReport,
};
pub use shared::{BucketName, CatalogId, PackageName};
pub use validation::Validate;
pub use version::Version;
