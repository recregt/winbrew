#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRef {
    ByName(String),
    ById(PackageId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageId {
    Winget { id: String },
    Scoop { bucket: String, id: String },
}

impl PackageRef {
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();

        if let Some(rest) = trimmed.strip_prefix('@') {
            Ok(Self::ById(PackageId::parse(rest)?))
        } else {
            Ok(Self::ByName(trimmed.to_string()))
        }
    }
}

impl PackageId {
    pub fn parse(input: &str) -> Result<Self, String> {
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
                let bucket = parts
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| invalid_package_id(trimmed))?;
                let id = parts
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| invalid_package_id(trimmed))?;

                if parts.next().is_some() {
                    return Err(invalid_package_id(trimmed));
                }

                Self::Scoop {
                    bucket: bucket.to_string(),
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
            Self::Scoop { bucket, id } => format!("scoop/{bucket}/{id}"),
        }
    }
}

fn invalid_package_id(input: &str) -> String {
    format!(
        "Invalid package ID '{input}'. Expected format:\n  @winget/<id>       e.g. @winget/Google.Chrome\n  @scoop/<bucket>/<id>  e.g. @scoop/main/7zip"
    )
}

#[cfg(test)]
mod tests {
    use super::{PackageId, PackageRef};

    #[test]
    fn parses_package_name() {
        assert_eq!(
            PackageRef::parse("git").unwrap(),
            PackageRef::ByName("git".to_string())
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
                bucket: "main".to_string(),
                id: "7zip".to_string(),
            })
        );
    }

    #[test]
    fn invalid_package_id_has_helpful_error() {
        let err = PackageRef::parse("@invalid").unwrap_err();

        assert!(err.contains("Expected format"));
        assert!(err.contains("@winget/Google.Chrome"));
        assert!(err.contains("@scoop/main/7zip"));
    }
}
