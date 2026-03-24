use anyhow::Result;

use crate::core::paths;

use super::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    pub database_path: String,
    pub database_exists: bool,
    pub install_root_source: String,
    pub install_root: String,
    pub install_root_exists: bool,
    pub packages_dir: String,
}

impl HealthReport {
    pub fn to_kv(&self) -> Vec<(String, String)> {
        vec![
            ("Database".to_string(), self.database_path.clone()),
            ("Database exists".to_string(), yes_no(self.database_exists)),
            (
                "Install root source".to_string(),
                self.install_root_source.clone(),
            ),
            ("Install root".to_string(), self.install_root.clone()),
            (
                "Install root exists".to_string(),
                yes_no(self.install_root_exists),
            ),
            ("Packages dir".to_string(), self.packages_dir.clone()),
        ]
    }
}

pub fn get_health_report() -> Result<HealthReport> {
    let config = Config::current();
    let paths = config.resolved_paths();

    Ok(HealthReport {
        database_path: paths::db_path().to_string_lossy().to_string(),
        database_exists: paths::db_path().exists(),
        install_root_source: "config:paths.root".to_string(),
        install_root: paths.root.to_string_lossy().to_string(),
        install_root_exists: paths.root.exists(),
        packages_dir: paths.packages.to_string_lossy().to_string(),
    })
}

fn yes_no(value: bool) -> String {
    if value {
        "yes".to_string()
    } else {
        "no".to_string()
    }
}
