use anyhow::Result;
use indicatif::ProgressBar;
use std::path::Path;
use std::time::Instant;

use crate::AppContext;

use super::scan;
use crate::models::{DiagnosisResult, DiagnosisSeverity, HealthReport, Package};

fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().into_owned()
}

fn sort_diagnostics(left: &DiagnosisResult, right: &DiagnosisResult) -> std::cmp::Ordering {
    left.severity
        .cmp(&right.severity)
        .then_with(|| left.error_code.cmp(&right.error_code))
        .then_with(|| left.description.cmp(&right.description))
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
pub fn health_report(ctx: &AppContext) -> Result<HealthReport> {
    let paths = &ctx.paths;
    let started_at = Instant::now();

    let (packages, mut diagnostics) = collect_packages(scan::installed_packages());
    let progress = (ctx.verbosity > 0).then(|| ProgressBar::new(packages.len() as u64));

    diagnostics.extend(scan::scan_packages_with_progress(
        &packages,
        progress.as_ref(),
    ));

    diagnostics.extend(scan::scan_orphaned_install_dirs(&paths.packages, &packages));
    diagnostics.sort_unstable_by(sort_diagnostics);

    let error_count = diagnostics
        .iter()
        .filter(|diagnosis| matches!(diagnosis.severity, DiagnosisSeverity::Error))
        .count();

    Ok(HealthReport {
        database_path: display_path(&paths.db),
        database_exists: paths.db.exists(),
        catalog_database_path: display_path(&paths.catalog_db),
        catalog_database_exists: paths.catalog_db.exists(),
        install_root_source: if ctx.root_from_env {
            "env override".to_string()
        } else {
            "config:paths.root".to_string()
        },
        install_root: display_path(&paths.root),
        install_root_exists: paths.root.exists(),
        packages_dir: display_path(&paths.packages),
        diagnostics,
        scan_duration: started_at.elapsed(),
        error_count,
    })
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
        let mut diagnostics = [
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
