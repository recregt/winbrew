use anyhow::Result;
use std::io::{self, Write};

use crate::commands::error::CommandError;
use crate::models::domains::reporting::{
    DiagnosisResult, DiagnosisSeverity, HealthReport, RecoveryActionGroup, RecoveryFinding,
};
use crate::{CommandContext, app::doctor};
use winbrew_ui::Ui;

/// Runs the system health check command.
///
/// When `json_output` is enabled, the report is written to stdout as JSON.
/// When `warn_as_error` is enabled, warnings produce a non-zero exit code.
pub fn run(ctx: &CommandContext, json_output: bool, warn_as_error: bool) -> Result<()> {
    if json_output {
        let report = doctor::health_report(ctx.app())?;
        let (_, warnings) = split_diagnostics(&report);

        let mut stdout = io::stdout();
        write_json(&mut stdout, &report)?;

        if let Some(exit_error) = exit_error(report.error_count, warnings.len(), warn_as_error) {
            return Err(exit_error);
        }

        return Ok(());
    }

    let mut ui = ctx.ui();
    ui.page_title("System Health Check");
    let report = ui.spinner("Inspecting environment and installed packages...", || {
        doctor::health_report(ctx.app())
    })?;
    let (errors, warnings) = split_diagnostics(&report);

    ui.display_key_values(&report_summary(&report));
    ui.info("");
    render_results(&mut ui, &errors, &warnings);
    ui.info("");
    render_recovery_preview(&mut ui, &report.recovery_findings);
    ui.info("Suggestion: Try running 'winbrew repair' or reinstalling the affected packages.");

    if let Some(exit_error) = exit_error(report.error_count, warnings.len(), warn_as_error) {
        return Err(exit_error);
    }

    Ok(())
}

fn report_summary(report: &HealthReport) -> Vec<(String, String)> {
    let timings = &report.scan_timings;

    vec![
        ("Database".to_string(), report.database_path.clone()),
        (
            "Database exists".to_string(),
            yes_no(report.database_exists).into(),
        ),
        (
            "Catalog database".to_string(),
            report.catalog_database_path.clone(),
        ),
        (
            "Catalog database exists".to_string(),
            yes_no(report.catalog_database_exists).into(),
        ),
        (
            "Install root source".to_string(),
            report.install_root_source.clone(),
        ),
        ("Install root".to_string(), report.install_root.clone()),
        (
            "Install root exists".to_string(),
            yes_no(report.install_root_exists).into(),
        ),
        ("Packages dir".to_string(), report.packages_dir.clone()),
        (
            "Scan duration".to_string(),
            format_duration(report.scan_duration),
        ),
        (
            "Database connection".to_string(),
            format_duration(timings.database_connection),
        ),
        (
            "Installed packages".to_string(),
            format_duration(timings.installed_packages),
        ),
        (
            "Package scan".to_string(),
            format_duration(timings.package_scan),
        ),
        ("MSI scan".to_string(), format_duration(timings.msi_scan)),
        (
            "Orphan scan".to_string(),
            format_duration(timings.orphan_scan),
        ),
        (
            "Journal scan".to_string(),
            format_duration(timings.journal_scan),
        ),
        (
            "Recovery findings".to_string(),
            report.recovery_findings.len().to_string(),
        ),
        ("Error count".to_string(), report.error_count.to_string()),
        (
            "Total findings".to_string(),
            report.diagnostics.len().to_string(),
        ),
    ]
}

fn split_diagnostics(report: &HealthReport) -> (Vec<&DiagnosisResult>, Vec<&DiagnosisResult>) {
    let mut errors = Vec::with_capacity(report.error_count);
    let mut warnings =
        Vec::with_capacity(report.diagnostics.len().saturating_sub(report.error_count));

    for entry in &report.diagnostics {
        match entry.severity {
            DiagnosisSeverity::Error => errors.push(entry),
            DiagnosisSeverity::Warning => warnings.push(entry),
        }
    }

    (errors, warnings)
}

/// Renders grouped diagnostics with errors first and warnings second.
pub fn render_results<W: std::io::Write>(
    ui: &mut Ui<W>,
    errors: &[&DiagnosisResult],
    warnings: &[&DiagnosisResult],
) {
    if errors.is_empty() && warnings.is_empty() {
        ui.success("Your Winbrew installation is healthy!");
        return;
    }

    ui.notice(format!("Issues found: {}", errors.len() + warnings.len()));

    if !errors.is_empty() {
        ui.error("Errors:");
        for entry in errors {
            ui.error(format!("  - [{}] {}", entry.error_code, entry.description));
        }
    }

    if !warnings.is_empty() {
        ui.warn("Warnings:");
        for entry in warnings {
            ui.warn(format!("  - [{}] {}", entry.error_code, entry.description));
        }
    }
}

fn render_recovery_preview<W: std::io::Write>(ui: &mut Ui<W>, findings: &[RecoveryFinding]) {
    let preview_lines = recovery_preview_lines(findings);

    if preview_lines.is_empty() {
        return;
    }

    ui.notice("Recovery preview:");
    for line in preview_lines {
        ui.info(format!("  - {line}"));
    }

    ui.info("");
}

fn recovery_preview_lines(findings: &[RecoveryFinding]) -> Vec<String> {
    let mut journal_replay = 0usize;
    let mut orphan_cleanup = 0usize;
    let mut file_restore = 0usize;
    let mut reinstall = 0usize;
    let mut manual_review = 0usize;

    for finding in findings {
        match finding.action_group {
            Some(RecoveryActionGroup::JournalReplay) => journal_replay += 1,
            Some(RecoveryActionGroup::OrphanCleanup) => orphan_cleanup += 1,
            Some(RecoveryActionGroup::FileRestore) => file_restore += 1,
            Some(RecoveryActionGroup::Reinstall) => reinstall += 1,
            None => manual_review += 1,
        }
    }

    let mut lines = Vec::new();
    push_recovery_preview_line(&mut lines, "Journal replay", journal_replay);
    push_recovery_preview_line(&mut lines, "Orphan cleanup", orphan_cleanup);
    push_recovery_preview_line(&mut lines, "File restore", file_restore);
    push_recovery_preview_line(&mut lines, "Reinstall", reinstall);
    push_recovery_preview_line(&mut lines, "Manual review", manual_review);

    lines
}

fn push_recovery_preview_line(lines: &mut Vec<String>, label: &str, count: usize) {
    if count == 0 {
        return;
    }

    lines.push(format!("{label}: {}", findings_label(count)));
}

fn findings_label(count: usize) -> String {
    format!("{count} finding{}", if count == 1 { "" } else { "s" })
}

/// Serializes the health report to JSON for machine consumption.
pub fn write_json<W: Write>(writer: &mut W, report: &HealthReport) -> Result<()> {
    serde_json::to_writer_pretty(&mut *writer, report)?;
    writeln!(writer)?;
    writer.flush()?;
    Ok(())
}

/// Renders a yes/no status as a static string.
fn yes_no(value: bool) -> &'static str {
    ["no", "yes"][value as usize]
}

/// Formats a [`std::time::Duration`] into a human-readable string.
pub fn format_duration(duration: std::time::Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else if duration.as_millis() > 0 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{}µs", duration.as_micros())
    }
}

pub fn exit_error(
    error_count: usize,
    warning_count: usize,
    warn_as_error: bool,
) -> Option<anyhow::Error> {
    if error_count > 0 {
        return Some(
            CommandError::reported(format!(
                "system health check found {} error(s)",
                error_count
            ))
            .with_exit_code(2)
            .into(),
        );
    }

    if warn_as_error && warning_count > 0 {
        return Some(
            CommandError::reported(format!(
                "system health check found {} warning(s)",
                warning_count
            ))
            .with_exit_code(1)
            .into(),
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domains::reporting::{
        DiagnosisSeverity, HealthReport, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind,
    };
    use crate::models::reporting::HealthScanTimings;
    use std::time::Duration;

    fn sample_report() -> HealthReport {
        HealthReport {
            database_path: "db.sqlite".to_string(),
            database_exists: true,
            catalog_database_path: "catalog.sqlite".to_string(),
            catalog_database_exists: true,
            install_root_source: "config:paths.root".to_string(),
            install_root: "C:/Tools".to_string(),
            install_root_exists: true,
            packages_dir: "C:/Tools/packages".to_string(),
            diagnostics: Vec::new(),
            recovery_findings: Vec::new(),
            scan_timings: HealthScanTimings {
                database_connection: Duration::from_micros(11),
                installed_packages: Duration::from_micros(12),
                package_scan: Duration::from_micros(13),
                msi_scan: Duration::from_micros(14),
                orphan_scan: Duration::from_micros(15),
                journal_scan: Duration::from_micros(16),
            },
            scan_duration: Duration::from_millis(99),
            error_count: 0,
        }
    }

    fn recovery_finding(
        error_code: &str,
        action_group: Option<RecoveryActionGroup>,
    ) -> RecoveryFinding {
        RecoveryFinding {
            error_code: error_code.to_string(),
            issue_kind: RecoveryIssueKind::RecoveryTrailMissing,
            action_group,
            description: error_code.to_string(),
            severity: DiagnosisSeverity::Warning,
            target_path: None,
        }
    }

    #[test]
    fn report_summary_includes_scan_timing_breakdown() {
        let summary = report_summary(&sample_report());

        assert!(
            summary
                .iter()
                .any(|(label, value)| { label == "Database connection" && value == "11µs" })
        );
        assert!(
            summary
                .iter()
                .any(|(label, value)| label == "Journal scan" && value == "16µs")
        );
    }

    #[test]
    fn recovery_preview_lines_groups_findings_by_action_group() {
        let findings = vec![
            recovery_finding("journal", Some(RecoveryActionGroup::JournalReplay)),
            recovery_finding("orphan", Some(RecoveryActionGroup::OrphanCleanup)),
            recovery_finding("manual", None),
        ];

        let lines = recovery_preview_lines(&findings);

        assert_eq!(
            lines,
            vec![
                "Journal replay: 1 finding".to_string(),
                "Orphan cleanup: 1 finding".to_string(),
                "Manual review: 1 finding".to_string(),
            ]
        );
    }
}
