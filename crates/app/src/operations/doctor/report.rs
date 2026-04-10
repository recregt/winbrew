use anyhow::Result;
use indicatif::ProgressBar;
use std::time::Instant;

use crate::AppContext;

use super::scan;
use crate::models::{DiagnosisResult, DiagnosisSeverity, HealthReport, Package};

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

        let (packages, mut diagnostics) = collect_packages(scan::installed_packages());
        let progress = if self.ctx.verbosity > 0 {
            ProgressBar::new(packages.len() as u64)
        } else {
            ProgressBar::hidden()
        };

        diagnostics.extend(scan::scan_packages_with_progress(&packages, &progress));
        if self.ctx.verbosity > 0 {
            progress.finish_and_clear();
        }

        diagnostics.extend(scan::scan_orphaned_install_dirs(&paths.packages, &packages));
        diagnostics.sort_unstable_by(|left, right| {
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

fn collect_packages(packages_result: Result<Vec<Package>>) -> (Vec<Package>, Vec<DiagnosisResult>) {
    match packages_result {
        Ok(packages) => (packages, Vec::new()),
        Err(err) => (
            Vec::new(),
            vec![DiagnosisResult {
                error_code: "installed_packages_unavailable".to_string(),
                description: format!("installed packages: unavailable ({err})"),
                severity: DiagnosisSeverity::Error,
            }],
        ),
    }
}

pub fn health_report(ctx: &AppContext) -> Result<HealthReport> {
    Reporter::new(ctx).collect()
}

#[cfg(test)]
mod tests {
    use super::collect_packages;
    use crate::models::DiagnosisSeverity;
    use anyhow::anyhow;

    #[test]
    fn collect_packages_converts_errors_into_diagnostics() {
        let (packages, diagnostics) = collect_packages(Err(anyhow!("database unavailable")));

        assert!(packages.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].error_code, "installed_packages_unavailable");
        assert_eq!(diagnostics[0].severity, DiagnosisSeverity::Error);
        assert!(diagnostics[0].description.contains("database unavailable"));
    }
}
