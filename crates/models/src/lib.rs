//! Typed model contracts for the Winbrew workspace.
//!
//! `winbrew-models` owns the stable Rust data types shared by the parser,
//! storage, engines, UI, and CLI layers. The crate is intentionally split into
//! a small set of domain families so consumers can import the exact concept
//! they need without depending on a broad compatibility surface.
//!
//! Public namespaces:
//!
//! - `shared`: errors, validation, identifiers, config, hash, and version
//! - `package`: package identity, queries, dependencies, and package aggregates
//! - `catalog`: typed catalog records and raw upstream catalog payloads
//! - `install`: installer metadata, engine receipts, installed state, and removal planning
//! - `reporting`: diagnostics, health reports, and recovery findings
//! - `msi_inventory`: MSI snapshot records used for repair and inventory persistence
//!
//! The `domains` facade remains as the stable grouping layer for downstream
//! callers. Inside this crate, prefer the owning module paths; outside the
//! crate, prefer `crate::domains::...` when a grouped namespace is clearer than
//! a direct module path.

pub mod catalog;
pub mod install;
pub mod msi_inventory;
pub mod package;
pub mod reporting;
pub mod shared;

/// Grouped namespace for the major model families.
pub mod domains {
    pub mod shared {
        pub use crate::shared::config::{ConfigSection, ConfigValue, ConfigValueSource};
        pub use crate::shared::error::ModelError;
        pub use crate::shared::hash::HashAlgorithm;
        pub use crate::shared::identifiers::{BucketName, CatalogId, PackageName};
        pub use crate::shared::validation::Validate;
        pub use crate::shared::version::Version;
    }

    pub mod package {
        pub use crate::package::dependency::Dependency;
        pub use crate::package::model::{Package, PackageKind, PackageSource};
        pub use crate::package::query::PackageQuery;
        pub use crate::package::reference::{PackageId, PackageRef};
        pub use crate::shared::identifiers::PackageName;
    }

    pub mod catalog {
        pub use crate::catalog::metadata::CatalogMetadata;
        pub use crate::catalog::package::{CatalogInstaller, CatalogPackage};
        pub use crate::catalog::raw::{RawCatalogInstaller, RawCatalogPackage};
    }

    pub mod installed {
        pub use crate::install::installed::{InstalledPackage, PackageStatus};
    }

    pub mod install {
        pub use crate::install::engine::{
            EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope,
        };
        pub use crate::install::installer::{Architecture, Installer, InstallerType};
        pub use crate::install::model::{InstallFailureClass, InstallOutcome, InstallResult};
        pub use crate::install::removal::RemovalPlan;
    }

    pub mod reporting {
        pub use crate::reporting::diagnostics::{DiagnosisResult, DiagnosisSeverity};
        pub use crate::reporting::info::InfoReport;
        pub use crate::reporting::report::{
            HealthReport, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind, ReportSection,
            RuntimeReport,
        };
    }

    pub mod inventory {
        pub use crate::msi_inventory::records::{
            MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
            MsiRegistryRecord, MsiShortcutRecord,
        };
    }
}
