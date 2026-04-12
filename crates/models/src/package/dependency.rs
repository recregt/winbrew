use serde::{Deserialize, Serialize};

use crate::shared::validation::{Validate, ensure_non_empty};
use crate::shared::{ModelError, Version};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub version: Option<Version>,
}

impl Dependency {
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
