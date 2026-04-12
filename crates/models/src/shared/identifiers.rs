use core::ops::Deref;
use core::str::FromStr;

use serde::{Deserialize, Serialize};

use super::ModelError;
use super::validation::{Validate, ensure_non_empty};
use crate::package_ref::PackageId;

macro_rules! define_string_newtype {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        field = $field:literal;
        parse = $parse:expr;
        validate = $validate:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        $vis struct $name(String);

        impl $name {
            pub fn parse(input: &str) -> Result<Self, ModelError> {
                let trimmed = input.trim();
                let value = ($parse)(trimmed)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Validate for $name {
            fn validate(&self) -> Result<(), ModelError> {
                ($validate)(&self.0)
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl FromStr for $name {
            type Err = ModelError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<String> for $name {
            fn eq(&self, other: &String) -> bool {
                &self.0 == other
            }
        }
    };
}

define_string_newtype! {
    pub struct CatalogId;
    field = "catalog_id";
    parse = |trimmed: &str| {
        if trimmed.is_empty() {
            return Err(ModelError::empty("catalog_id"));
        }

        PackageId::parse(trimmed)?;
        Ok(trimmed.to_string())
    };
    validate = |value: &str| {
        ensure_non_empty("catalog_id", value)?;
        PackageId::parse(value).map(|_| ())
    };
}

define_string_newtype! {
    pub struct PackageName;
    field = "package_ref.name";
    parse = |trimmed: &str| {
        if trimmed.is_empty() {
            return Err(ModelError::empty("package_ref.name"));
        }

        Ok(trimmed.to_string())
    };
    validate = |value: &str| ensure_non_empty("package_ref.name", value);
}

define_string_newtype! {
    pub struct BucketName;
    field = "package_id.bucket";
    parse = |trimmed: &str| {
        if trimmed.is_empty() {
            return Err(ModelError::empty("package_id.bucket"));
        }

        if trimmed.contains('/') {
            return Err(ModelError::invalid_package_id(
                trimmed,
                "bucket names cannot contain '/'",
            ));
        }

        Ok(trimmed.to_string())
    };
    validate = |value: &str| {
        ensure_non_empty("package_id.bucket", value)?;

        if value.contains('/') {
            return Err(ModelError::invalid_package_id(
                value,
                "bucket names cannot contain '/'",
            ));
        }

        Ok(())
    };
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
