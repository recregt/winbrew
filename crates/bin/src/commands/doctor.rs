use anyhow::Result;
use std::io::{self, Write};

use crate::commands::error::CommandError;
use crate::models::{DiagnosisSeverity, HealthReport};
use crate::{AppContext, Ui, services::app::doctor};

/// Runs the system health check command.
///
/// When `json_output` is enabled, the report is written to stdout as JSON.
/// When `warn_as_error` is enabled, warnings produce a non-zero exit code.
pub fn run(ctx: &AppContext, json_output: bool, warn_as_error: bool) -> Result<()> {
    let report = doctor::health_report(ctx)?;
    let diagnostics = group_diagnostics(&report);

    if json_output {
        let mut stdout = io::stdout();
        write_json(&mut stdout, &report)?;

        if let Some(exit_error) = exit_error(&report, diagnostics.warning_count(), warn_as_error) {
            return Err(exit_error);
        }

        return Ok(());
    }

    let mut ui = Ui::new(ctx.ui);
    ui.page_title("System Health Check");
    ui.info("Inspecting environment and installed packages...");
    ui.display_key_values(&report_summary(&report));
    ui.info("");
    render_results(&mut ui, &diagnostics, &report);

    if let Some(exit_error) = exit_error(&report, diagnostics.warning_count(), warn_as_error) {
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
        ("Error count".to_string(), report.error_count.to_string()),
        (
            "Total findings".to_string(),
            report.diagnostics.len().to_string(),
        ),
    ]
}

struct DiagnosticGroups<'a> {
    errors: Vec<&'a crate::models::DiagnosisResult>,
    warnings: Vec<&'a crate::models::DiagnosisResult>,
}

impl<'a> DiagnosticGroups<'a> {
    fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}

fn group_diagnostics(report: &HealthReport) -> DiagnosticGroups<'_> {
    let mut errors = Vec::with_capacity(report.error_count);
    let mut warnings =
        Vec::with_capacity(report.diagnostics.len().saturating_sub(report.error_count));

    for entry in &report.diagnostics {
        match entry.severity {
            DiagnosisSeverity::Error => errors.push(entry),
            DiagnosisSeverity::Warning => warnings.push(entry),
        }
    }

    DiagnosticGroups { errors, warnings }
}

/// Renders grouped diagnostics with errors first and warnings second.
fn render_results<W: std::io::Write>(
    ui: &mut Ui<W>,
    diagnostics: &DiagnosticGroups<'_>,
    report: &HealthReport,
) {
    if report.diagnostics.is_empty() {
        ui.success("Your Winbrew installation is healthy!");
        return;
    }

    ui.notice(format!("Issues found: {}", report.diagnostics.len()));

    if !diagnostics.errors.is_empty() {
        ui.error("Errors:");
        for entry in &diagnostics.errors {
            ui.error(format!("  - [{}] {}", entry.error_code, entry.description));
        }
    }

    if !diagnostics.warnings.is_empty() {
        ui.warn("Warnings:");
        for entry in &diagnostics.warnings {
            ui.warn(format!("  - [{}] {}", entry.error_code, entry.description));
        }
    }

    ui.info("");
    ui.info("Suggestion: Try running 'winbrew repair' or reinstalling the affected packages.");
}

/// Serializes the health report to JSON for machine consumption.
fn write_json<W: Write>(writer: &mut W, report: &HealthReport) -> Result<()> {
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
fn format_duration(duration: std::time::Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else if duration.as_millis() > 0 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{}µs", duration.as_micros())
    }
}

fn exit_error(
    report: &HealthReport,
    warning_count: usize,
    warn_as_error: bool,
) -> Option<anyhow::Error> {
    if report.error_count > 0 {
        return Some(
            CommandError::reported(format!(
                "system health check found {} error(s)",
                report.error_count
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
    use super::{exit_error, format_duration, group_diagnostics, render_results, write_json};
    use crate::commands::error::CommandError;
    use crate::models::{DiagnosisResult, DiagnosisSeverity, HealthReport};
    use std::io::{Result as IoResult, Write};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use winbrew_ui::{UiBuilder, UiSettings};

    struct SharedBuffer {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl SharedBuffer {
        fn new(bytes: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { bytes }
        }
    }

    impl Write for SharedBuffer {
        fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
            let mut bytes = self.bytes.lock().expect("buffer lock should be available");
            bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    fn diagnosis(
        error_code: &str,
        description: &str,
        severity: DiagnosisSeverity,
    ) -> DiagnosisResult {
        DiagnosisResult {
            error_code: error_code.to_string(),
            description: description.to_string(),
            severity,
        }
    }

    fn sample_report(diagnostics: Vec<DiagnosisResult>) -> HealthReport {
        let error_count = diagnostics
            .iter()
            .filter(|diagnosis| diagnosis.severity == DiagnosisSeverity::Error)
            .count();

        HealthReport {
            database_path: "C:\\Users\\Test\\AppData\\Local\\winbrew\\data\\winbrew.db".to_string(),
            database_exists: true,
            catalog_database_path: "C:\\Users\\Test\\AppData\\Local\\winbrew\\data\\catalog.db"
                .to_string(),
            catalog_database_exists: true,
            install_root_source: "config:paths.root".to_string(),
            install_root: "C:\\Users\\Test\\AppData\\Local\\winbrew".to_string(),
            install_root_exists: true,
            packages_dir: "C:\\Users\\Test\\AppData\\Local\\winbrew\\packages".to_string(),
            diagnostics,
            scan_duration: Duration::from_millis(1_234),
            error_count,
        }
    }

    #[test]
    fn render_results_groups_errors_before_warnings() {
        let shared_bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer::new(Arc::clone(&shared_bytes));
        let err_writer = SharedBuffer::new(Arc::clone(&shared_bytes));
        let mut ui = UiBuilder::with_writer(
            writer,
            UiSettings {
                color_enabled: false,
                default_yes: false,
            },
        )
        .with_error_writer(Box::new(err_writer))
        .build();

        let report = sample_report(vec![
            diagnosis(
                "orphan_install_directory",
                "orphaned package",
                DiagnosisSeverity::Warning,
            ),
            diagnosis(
                "missing_install_directory",
                "missing directory",
                DiagnosisSeverity::Error,
            ),
        ]);

        let groups = group_diagnostics(&report);
        render_results(&mut ui, &groups, &report);

        let output = String::from_utf8(
            shared_bytes
                .lock()
                .expect("buffer lock should be available")
                .clone(),
        )
        .expect("rendered output should be valid UTF-8");

        let error_position = output
            .find("✘ Errors:")
            .expect("errors heading should exist");
        let warning_position = output
            .find("⚠ Warnings:")
            .expect("warnings heading should exist");

        assert!(error_position < warning_position);
        assert!(output.contains("✘   - [missing_install_directory] missing directory"));
        assert!(output.contains("⚠   - [orphan_install_directory] orphaned package"));
    }

    #[test]
    fn render_results_handles_empty_diagnostics() {
        let shared_bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer::new(Arc::clone(&shared_bytes));
        let err_writer = SharedBuffer::new(Arc::clone(&shared_bytes));
        let mut ui = UiBuilder::with_writer(
            writer,
            UiSettings {
                color_enabled: false,
                default_yes: false,
            },
        )
        .with_error_writer(Box::new(err_writer))
        .build();

        let report = sample_report(Vec::new());
        let groups = group_diagnostics(&report);

        render_results(&mut ui, &groups, &report);

        let output = String::from_utf8(
            shared_bytes
                .lock()
                .expect("buffer lock should be available")
                .clone(),
        )
        .expect("rendered output should be valid UTF-8");

        assert!(output.contains("Your Winbrew installation is healthy!"));
        assert!(!output.contains("Errors:"));
        assert!(!output.contains("Warnings:"));
    }

    #[test]
    fn format_duration_uses_seconds_milliseconds_and_microseconds() {
        assert_eq!(format_duration(Duration::from_secs(2)), "2.00s");
        assert_eq!(format_duration(Duration::from_millis(15)), "15ms");
        assert_eq!(format_duration(Duration::from_micros(7)), "7µs");
        assert_eq!(format_duration(Duration::from_micros(0)), "0µs");
    }

    #[test]
    fn write_json_serializes_health_report() {
        let report = sample_report(vec![diagnosis(
            "missing_install_directory",
            "missing directory",
            DiagnosisSeverity::Error,
        )]);

        let mut output = Vec::new();
        write_json(&mut output, &report).expect("json should serialize");

        let value: serde_json::Value = serde_json::from_slice(&output).expect("json should parse");

        assert_eq!(value["scan_duration"], 1234);
        assert_eq!(value["diagnostics"][0]["severity"], "error");
    }

    #[test]
    fn write_json_serializes_empty_diagnostics() {
        let report = sample_report(Vec::new());

        let mut output = Vec::new();
        write_json(&mut output, &report).expect("json should serialize");

        let value: serde_json::Value = serde_json::from_slice(&output).expect("json should parse");

        assert!(
            value["diagnostics"]
                .as_array()
                .is_some_and(|entries| entries.is_empty())
        );
    }

    #[test]
    fn exit_error_uses_distinct_codes_for_errors_and_warnings() {
        let error_report = sample_report(vec![
            diagnosis(
                "missing_install_directory-1",
                "missing directory",
                DiagnosisSeverity::Error,
            ),
            diagnosis(
                "missing_install_directory-2",
                "missing directory",
                DiagnosisSeverity::Error,
            ),
            diagnosis(
                "missing_install_directory-3",
                "missing directory",
                DiagnosisSeverity::Error,
            ),
        ]);

        let error = exit_error(&error_report, 0, false).expect("error code should be returned");
        let cmd_error = error
            .downcast_ref::<CommandError>()
            .expect("command error should exist");
        assert_eq!(cmd_error.exit_code(), 2);

        let warning_report = sample_report(vec![diagnosis(
            "orphan_install_directory",
            "orphaned package",
            DiagnosisSeverity::Warning,
        )]);
        let warning =
            exit_error(&warning_report, 1, true).expect("warning exit code should be returned");
        let cmd_error = warning
            .downcast_ref::<CommandError>()
            .expect("command error should exist");
        assert_eq!(cmd_error.exit_code(), 1);

        assert!(exit_error(&warning_report, 1, false).is_none());
        assert!(exit_error(&warning_report, 0, false).is_none());
    }
}
