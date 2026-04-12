use anyhow::Result;
use std::io::{self, Write};

use crate::commands::error::CommandError;
use crate::models::{DiagnosisResult, DiagnosisSeverity, HealthReport};
use crate::{CommandContext, app::doctor};
use winbrew_ui::Ui;

/// Runs the system health check command.
///
/// When `json_output` is enabled, the report is written to stdout as JSON.
/// When `warn_as_error` is enabled, warnings produce a non-zero exit code.
pub fn run(ctx: &CommandContext, json_output: bool, warn_as_error: bool) -> Result<()> {
    if json_output {
        let report = doctor::health_report(ctx)?;
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
        doctor::health_report(ctx)
    })?;
    let (errors, warnings) = split_diagnostics(&report);

    ui.display_key_values(&report_summary(&report));
    ui.info("");
    render_results(&mut ui, &errors, &warnings);

    if let Some(exit_error) = exit_error(report.error_count, warnings.len(), warn_as_error) {
        return Err(exit_error);
    }

    Ok(())
}

fn report_summary(report: &HealthReport) -> Vec<(String, String)> {
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

    ui.info("");
    ui.info("Suggestion: Try running 'winbrew repair' or reinstalling the affected packages.");
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
