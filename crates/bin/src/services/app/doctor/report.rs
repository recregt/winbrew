use anyhow::Result;
use indicatif::ProgressBar;
use std::time::Instant;

use crate::AppContext;

use super::scan;
use crate::models::{DiagnosisSeverity, HealthReport};

pub struct Reporter<'a> {
    ctx: &'a AppContext,
}

impl<'a> Reporter<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub fn collect(&self) -> Result<HealthReport> {
        let paths = &self.ctx.paths;
        let started_at = Instant::now();

        let packages = scan::installed_packages()?;
        let progress = ProgressBar::hidden();
        let mut diagnostics = scan::scan_packages_with_progress(&packages, &progress);
        diagnostics.extend(scan::scan_orphaned_install_dirs(&paths.packages, &packages));
        diagnostics.sort_by(|left, right| {
            left.error_code
                .cmp(&right.error_code)
                .then_with(|| left.description.cmp(&right.description))
                .then_with(|| left.severity.cmp(&right.severity))
        });

        let error_count = diagnostics
            .iter()
            .filter(|diagnosis| diagnosis.severity == DiagnosisSeverity::Error)
            .count();

        Ok(HealthReport {
            database_path: paths.db.to_string_lossy().to_string(),
            database_exists: paths.db.exists(),
            catalog_database_path: paths.catalog_db.to_string_lossy().to_string(),
            catalog_database_exists: paths.catalog_db.exists(),
            install_root_source: if self.ctx.root_from_env {
                "env override".to_string()
            } else {
                "config:paths.root".to_string()
            },
            install_root: paths.root.to_string_lossy().to_string(),
            install_root_exists: paths.root.exists(),
            packages_dir: paths.packages.to_string_lossy().to_string(),
            diagnostics,
            scan_duration: started_at.elapsed(),
            error_count,
        })
    }
}

pub fn health_report(ctx: &AppContext) -> Result<HealthReport> {
    Reporter::new(ctx).collect()
}
