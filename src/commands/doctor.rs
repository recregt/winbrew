use anyhow::Result;

use crate::{
    AppContext,
    models::{HealthReport, diagnostics::DiagnosisSeverity},
    services::app::doctor,
    ui::Ui,
};

pub fn run(ctx: &AppContext) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("System Health Check");
    ui.info("Inspecting environment and installed packages...");
    let report = doctor::health_report(ctx)?;
    ui.display_key_values(&report_summary(&report));
    ui.info("");
    render_results(&mut ui, &report);

    Ok(())
}

fn report_summary(report: &HealthReport) -> Vec<(String, String)> {
    vec![
        ("Database".to_string(), report.database_path.clone()),
        (
            "Database exists".to_string(),
            yes_no(report.database_exists),
        ),
        (
            "Catalog database".to_string(),
            report.catalog_database_path.clone(),
        ),
        (
            "Catalog database exists".to_string(),
            yes_no(report.catalog_database_exists),
        ),
        (
            "Install root source".to_string(),
            report.install_root_source.clone(),
        ),
        ("Install root".to_string(), report.install_root.clone()),
        (
            "Install root exists".to_string(),
            yes_no(report.install_root_exists),
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

fn render_results<W: std::io::Write>(ui: &mut Ui<W>, report: &HealthReport) {
    if report.diagnostics.is_empty() {
        ui.success("Your Winbrew installation is healthy!");
        return;
    }

    ui.notice(format!("Broken installs: {}", report.diagnostics.len()));
    ui.warn("Found the following issues:");
    for entry in &report.diagnostics {
        ui.notice(format!(
            "  - [{}] {} ({})",
            entry.error_code,
            entry.description,
            severity_label(entry.severity)
        ));
    }
    ui.info("");
    ui.info("Suggestion: Try running 'winbrew repair' or reinstalling the affected packages.");
}

fn yes_no(value: bool) -> String {
    if value {
        "yes".to_string()
    } else {
        "no".to_string()
    }
}

fn format_duration(duration: std::time::Duration) -> String {
    let millis = duration.as_millis();
    format!("{millis}ms")
}

fn severity_label(severity: DiagnosisSeverity) -> &'static str {
    match severity {
        DiagnosisSeverity::Error => "error",
        DiagnosisSeverity::Warning => "warning",
    }
}
