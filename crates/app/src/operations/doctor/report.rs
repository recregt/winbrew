use anyhow::Result;
use indicatif::ProgressBar;
use std::path::Path;
use std::time::Instant;

use crate::AppContext;

use super::scan;
use crate::models::{DiagnosisResult, DiagnosisSeverity, HealthReport, Package};

/// Builds a health report from the application context and the current install state.
///
/// `Reporter` owns no data itself; it is a thin wrapper around [`AppContext`] so the
/// doctor pipeline can read resolved paths, verbosity, and root provenance without
/// threading those values through every helper function.
pub struct Reporter<'a> {
    ctx: &'a AppContext,
}

impl<'a> Reporter<'a> {
    /// Creates a reporter bound to a shared application context.
    ///
    /// The context provides resolved filesystem paths, UI settings, and CLI-derived
    /// runtime flags used while assembling the report.
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    /// Collects the current health state into a [`HealthReport`].
    ///
    /// The collection flow performs three steps:
    /// 1. Read the installed package inventory.
    /// 2. Scan package install directories and orphaned package directories.
    /// 3. Normalize and sort diagnostics so the UI can render a stable report.
    ///
    /// If package inventory access fails, the failure is converted into a synthetic
    /// diagnosis so the report can still be returned with partial data.
    pub fn collect(&self) -> Result<HealthReport> {
        let paths = &self.ctx.paths;
        let started_at = Instant::now();

        let (packages, mut diagnostics) = collect_packages(scan::installed_packages());
        let progress = (self.ctx.verbosity > 0).then(|| ProgressBar::new(packages.len() as u64));

        diagnostics.extend(scan::scan_packages_with_progress(
            &packages,
            progress.as_ref(),
        ));
        if let Some(progress) = progress.as_ref() {
            progress.finish_and_clear();
        }

        diagnostics.extend(scan::scan_orphaned_install_dirs(&paths.packages, &packages));
        diagnostics.sort_unstable_by(sort_diagnostics);

        let error_count = diagnostics
            .iter()
            .filter(|diagnosis| matches!(diagnosis.severity, DiagnosisSeverity::Error))
            .count();

        Ok(HealthReport {
            database_path: paths.db.to_display(),
            database_exists: paths.db.exists(),
            catalog_database_path: paths.catalog_db.to_display(),
            catalog_database_exists: paths.catalog_db.exists(),
            install_root_source: if self.ctx.root_from_env {
                "env override".to_string()
            } else {
                "config:paths.root".to_string()
            },
            install_root: paths.root.to_display(),
            install_root_exists: paths.root.exists(),
            packages_dir: paths.packages.to_display(),
            diagnostics,
            scan_duration: started_at.elapsed(),
            error_count,
        })
    }
}

trait PathDisplay {
    fn to_display(&self) -> String;
}

fn sort_diagnostics(left: &DiagnosisResult, right: &DiagnosisResult) -> std::cmp::Ordering {
    left.severity
        .cmp(&right.severity)
        .then_with(|| left.error_code.cmp(&right.error_code))
        .then_with(|| left.description.cmp(&right.description))
}

impl<T> PathDisplay for T
where
    T: AsRef<Path>,
{
    fn to_display(&self) -> String {
        self.as_ref().to_string_lossy().into_owned()
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

/// Convenience entry point for callers that need a one-shot health report.
///
/// This delegates to [`Reporter::collect`] so the caller does not need to manage the
/// reporter wrapper directly.
pub fn health_report(ctx: &AppContext) -> Result<HealthReport> {
    Reporter::new(ctx).collect()
}

#[cfg(test)]
mod tests {
    use super::{collect_packages, sort_diagnostics};
    use crate::models::{DiagnosisResult, DiagnosisSeverity};
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

    #[test]
    fn sort_diagnostics_keeps_errors_before_warnings() {
        let mut diagnostics = vec![
            DiagnosisResult {
                error_code: "warning_b".to_string(),
                description: "warning".to_string(),
                severity: DiagnosisSeverity::Warning,
            },
            DiagnosisResult {
                error_code: "error_a".to_string(),
                description: "error".to_string(),
                severity: DiagnosisSeverity::Error,
            },
            DiagnosisResult {
                error_code: "error_c".to_string(),
                description: "error".to_string(),
                severity: DiagnosisSeverity::Error,
            },
        ];

        diagnostics.sort_unstable_by(sort_diagnostics);

        assert_eq!(diagnostics[0].severity, DiagnosisSeverity::Error);
        assert_eq!(diagnostics[1].severity, DiagnosisSeverity::Error);
        assert_eq!(diagnostics[2].severity, DiagnosisSeverity::Warning);
    }

    #[test]
    fn collect_packages_keeps_empty_package_lists_empty() {
        let (packages, diagnostics) = collect_packages(Ok(Vec::new()));

        assert!(packages.is_empty());
        assert!(diagnostics.is_empty());
    }
}
