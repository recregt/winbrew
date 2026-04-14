use crate::models::domains::install::{EngineKind, EngineMetadata, InstallerType};
use crate::models::domains::installed::{InstalledPackage, PackageStatus};
use std::path::Path;

pub const DEFAULT_INSTALLED_AT: &str = "2026-04-12T00:00:00Z";

pub struct InstalledPackageBuilder {
    name: String,
    version: String,
    kind: InstallerType,
    status: PackageStatus,
    installed_at: String,
    dependencies: Vec<String>,
    engine_metadata: Option<EngineMetadata>,
}

impl InstalledPackageBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            status: PackageStatus::Ok,
            installed_at: DEFAULT_INSTALLED_AT.to_string(),
            dependencies: Vec::new(),
            engine_metadata: None,
        }
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn kind(mut self, kind: InstallerType) -> Self {
        self.kind = kind;
        self
    }

    pub fn status(mut self, status: PackageStatus) -> Self {
        self.status = status;
        self
    }

    pub fn installed_at(mut self, installed_at: impl Into<String>) -> Self {
        self.installed_at = installed_at.into();
        self
    }

    pub fn dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn engine_metadata(mut self, engine_metadata: Option<EngineMetadata>) -> Self {
        self.engine_metadata = engine_metadata;
        self
    }

    pub fn build(self, install_dir: &Path) -> InstalledPackage {
        InstalledPackage {
            name: self.name,
            version: self.version,
            kind: self.kind,
            deployment_kind: self.kind.deployment_kind(),
            engine_kind: EngineKind::from(self.kind),
            engine_metadata: self.engine_metadata,
            install_dir: install_dir.to_string_lossy().to_string(),
            dependencies: self.dependencies,
            status: self.status,
            installed_at: self.installed_at,
        }
    }
}
