use core::fmt;
use core::str::FromStr;

use semver as semver_crate;
use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::validation::Validate;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Version(semver_crate::Version);

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(semver_crate::Version::new(major, minor, patch))
    }

    pub fn parse(value: &str) -> Result<Self, ModelError> {
        value.parse()
    }

    pub fn as_semver(&self) -> &semver_crate::Version {
        &self.0
    }
}

impl FromStr for Version {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        semver_crate::Version::parse(s)
            .map(Self)
            .map_err(|err| ModelError::invalid_version(s, err.to_string()))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<semver_crate::Version> for Version {
    fn from(value: semver_crate::Version) -> Self {
        Self(value)
    }
}

impl From<Version> for semver_crate::Version {
    fn from(value: Version) -> Self {
        value.0
    }
}

impl From<Version> for String {
    fn from(value: Version) -> Self {
        value.to_string()
    }
}

impl Validate for Version {
    fn validate(&self) -> Result<(), ModelError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Version;

    #[test]
    fn parses_semver_and_orders_versions() {
        let version = Version::parse("1.2.3").expect("version should parse");
        let newer = Version::parse("1.2.4").expect("version should parse");

        assert!(newer > version);
        assert_eq!(version.to_string(), "1.2.3");
    }
}
