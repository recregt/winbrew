use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::identifiers::{BucketName, PackageName};
use crate::validation::{Validate, ensure_non_empty};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageRef {
    ByName(PackageName),
    ById(PackageId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageId {
    Winget { id: String },
    Scoop { bucket: BucketName, id: String },
}

impl PackageRef {
    pub fn parse(input: &str) -> Result<Self, ModelError> {
        let trimmed = input.trim();

        if let Some(rest) = trimmed.strip_prefix('@') {
            Ok(Self::ById(PackageId::parse(rest)?))
        } else if trimmed.is_empty() {
            Err(ModelError::empty("package_ref"))
        } else {
            Ok(Self::ByName(PackageName::parse(trimmed)?))
        }
    }
}

impl Validate for PackageRef {
    fn validate(&self) -> Result<(), ModelError> {
        match self {
            Self::ByName(name) => name.validate(),
            Self::ById(package_id) => package_id.validate(),
        }
    }
}

impl PackageId {
    pub fn parse(input: &str) -> Result<Self, ModelError> {
        let trimmed = input.trim();
        let mut parts = trimmed.split('/');

        let source = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid_package_id(trimmed))?;

        let package_id = match source {
            "winget" => {
                let id = parts
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| invalid_package_id(trimmed))?;

                if parts.next().is_some() {
                    return Err(invalid_package_id(trimmed));
                }

                Self::Winget { id: id.to_string() }
            }
            "scoop" => {
                let bucket = BucketName::parse(
                    parts
                        .next()
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| invalid_package_id(trimmed))?,
                )?;
                let id = parts
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| invalid_package_id(trimmed))?;

                if parts.next().is_some() {
                    return Err(invalid_package_id(trimmed));
                }

                Self::Scoop {
                    bucket,
                    id: id.to_string(),
                }
            }
            _ => return Err(invalid_package_id(trimmed)),
        };

        Ok(package_id)
    }

    pub fn catalog_id(&self) -> String {
        match self {
            Self::Winget { id } => format!("winget/{id}"),
            Self::Scoop { bucket, id } => format!("scoop/{}/{id}", bucket.as_str()),
        }
    }
}

impl Validate for PackageId {
    fn validate(&self) -> Result<(), ModelError> {
        match self {
            Self::Winget { id } => ensure_non_empty("package_id.id", id),
            Self::Scoop { bucket, id } => {
                bucket.validate()?;
                ensure_non_empty("package_id.id", id)
            }
        }
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.catalog_id())
    }
}

impl FromStr for PackageRef {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl FromStr for PackageId {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

fn invalid_package_id(input: &str) -> ModelError {
    ModelError::invalid_package_id(input, "expected @winget/<id> or @scoop/<bucket>/<id>")
}

#[cfg(test)]
mod tests {
    use super::{BucketName, PackageId, PackageRef};
    use crate::identifiers::PackageName;

    #[test]
    fn parses_package_name() {
        assert_eq!(
            PackageRef::parse("git").unwrap(),
            PackageRef::ByName(PackageName::parse("git").unwrap())
        );
    }

    #[test]
    fn parses_winget_id() {
        assert_eq!(
            PackageRef::parse("@winget/Google.Chrome").unwrap(),
            PackageRef::ById(PackageId::Winget {
                id: "Google.Chrome".to_string(),
            })
        );
    }

    #[test]
    fn parses_scoop_id() {
        assert_eq!(
            PackageRef::parse("@scoop/main/7zip").unwrap(),
            PackageRef::ById(PackageId::Scoop {
                bucket: BucketName::parse("main").unwrap(),
                id: "7zip".to_string(),
            })
        );
    }

    #[test]
    fn rejects_bucket_names_with_slashes() {
        let err = BucketName::parse("main/tools").unwrap_err();

        assert!(err.to_string().contains("bucket names cannot contain '/'"));
    }

    #[test]
    fn invalid_package_id_has_helpful_error() {
        let err = PackageRef::parse("@invalid").unwrap_err();

        assert!(
            err.to_string()
                .contains("expected @winget/<id> or @scoop/<bucket>/<id>")
        );
    }
}
