use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PackageStatus {
    Installing,
    Ok,
    Updating,
    Failed,
}

impl PackageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Installing => "installing",
            Self::Ok => "ok",
            Self::Updating => "updating",
            Self::Failed => "failed",
        }
    }

    pub fn parse(status: &str) -> Self {
        match status {
            "ok" => Self::Ok,
            "updating" => Self::Updating,
            "failed" => Self::Failed,
            _ => Self::Installing,
        }
    }
}

impl std::fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub kind: String,
    pub install_dir: String,
    pub product_code: Option<String>,
    pub dependencies: Vec<String>,
    pub status: PackageStatus,
    pub installed_at: String,
}

#[derive(Debug, Clone)]
pub struct PackageQuery {
    pub terms: Vec<String>,
    pub version: Option<String>,
}

impl PackageQuery {
    pub fn text(&self) -> String {
        self.terms.join(" ")
    }
}
