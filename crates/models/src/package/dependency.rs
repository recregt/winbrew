use serde::{Deserialize, Serialize};

use crate::shared::validation::{Validate, ensure_non_empty};
use crate::shared::{ModelError, Version};

/// A single dependency entry declared by a package.
///
/// Dependencies are stored as package ids plus an optional minimum version so
/// downstream code can reason about ordering and compatibility without parsing
/// source-specific metadata again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// The dependency package id or identifier.
    pub id: String,
    /// Optional minimum version constraint for the dependency.
    pub version: Option<Version>,
}

impl Dependency {
    /// Validate the dependency id and any attached version constraint.
    pub fn validate(&self) -> Result<(), ModelError> {
        ensure_non_empty("dependency.id", &self.id)?;
        if let Some(version) = &self.version {
            version.validate()?;
        }
        Ok(())
    }
}

impl Validate for Dependency {
    fn validate(&self) -> Result<(), ModelError> {
        Dependency::validate(self)
    }
}
