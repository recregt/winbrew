use core::ops::Deref;
use core::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::package_ref::PackageId;
use crate::validation::{Validate, ensure_non_empty};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CatalogId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageName(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BucketName(String);

impl CatalogId {
    pub fn parse(input: &str) -> Result<Self, ModelError> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Err(ModelError::empty("catalog_id"));
        }

        PackageId::parse(trimmed)?;
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PackageName {
    pub fn parse(input: &str) -> Result<Self, ModelError> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Err(ModelError::empty("package_ref.name"));
        }

        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl BucketName {
    pub fn parse(input: &str) -> Result<Self, ModelError> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Err(ModelError::empty("package_id.bucket"));
        }

        if trimmed.contains('/') {
            return Err(ModelError::invalid_package_id(
                trimmed,
                "bucket names cannot contain '/'",
            ));
        }

        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Validate for CatalogId {
    fn validate(&self) -> Result<(), ModelError> {
        ensure_non_empty("catalog_id", &self.0)?;
        PackageId::parse(&self.0).map(|_| ())
    }
}

impl Validate for PackageName {
    fn validate(&self) -> Result<(), ModelError> {
        ensure_non_empty("package_ref.name", &self.0)
    }
}

impl Validate for BucketName {
    fn validate(&self) -> Result<(), ModelError> {
        ensure_non_empty("package_id.bucket", &self.0)?;

        if self.0.contains('/') {
            return Err(ModelError::invalid_package_id(
                &self.0,
                "bucket names cannot contain '/'",
            ));
        }

        Ok(())
    }
}

impl Deref for CatalogId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Deref for PackageName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Deref for BucketName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl core::fmt::Display for CatalogId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::fmt::Display for PackageName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::fmt::Display for BucketName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CatalogId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for PackageName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for BucketName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for CatalogId {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl FromStr for PackageName {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl FromStr for BucketName {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<String> for CatalogId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CatalogId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<PackageId> for CatalogId {
    fn from(value: PackageId) -> Self {
        Self(value.catalog_id())
    }
}

impl From<&PackageId> for CatalogId {
    fn from(value: &PackageId) -> Self {
        Self(value.catalog_id())
    }
}

impl From<String> for PackageName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for PackageName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for BucketName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for BucketName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl PartialEq<&str> for CatalogId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for CatalogId {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<&str> for PackageName {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for PackageName {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<&str> for BucketName {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for BucketName {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

#[cfg(test)]
mod tests {
    use super::{BucketName, CatalogId, PackageName};

    #[test]
    fn parses_catalog_ids() {
        let id = CatalogId::parse("winget/Contoso.App").expect("catalog id should parse");

        assert_eq!(id.as_str(), "winget/Contoso.App");
    }

    #[test]
    fn parses_non_empty_package_name() {
        let name = PackageName::parse("Contoso App").expect("package name should parse");

        assert_eq!(name.as_str(), "Contoso App");
    }

    #[test]
    fn parses_bucket_name() {
        let bucket = BucketName::parse("main").expect("bucket should parse");

        assert_eq!(bucket.as_str(), "main");
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(CatalogId::parse("invalid").is_err());
        assert!(PackageName::parse("   ").is_err());
        assert!(BucketName::parse("main/tools").is_err());
    }
}
