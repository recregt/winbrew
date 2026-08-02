//! Info command wrapper for runtime configuration reporting.
//!
//! The wrapper formats the collected runtime settings and prints the report
//! sections in the order returned by the app layer.

use anyhow::Result;
use std::io::Write;

use crate::{
    CommandContext,
    app::{AppContext, info},
};
use winbrew_app::models::domains::reporting::{InfoReport, ReportSection, RuntimeReport};
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext) -> Result<()> {
    let mut ui = ctx.ui();
    run_with_ui(&mut ui, ctx.app())
}

fn run_with_ui<W: Write>(ui: &mut Ui<W>, app: &AppContext) -> Result<()> {
    let report = info::collect(&app.sections, &app.paths)?;
    render_info_report(ui, &report);

    Ok(())
}

fn render_info_report<W: Write>(ui: &mut Ui<W>, report: &InfoReport) {
    ui.write_line(format!("WinBrew Package Manager v{}", report.version));
    ui.write_line("Copyright (c) 2026 The WinBrew Contributors.");
    ui.write_line("Licensed under either of MIT or Apache 2.0 at your option.");
    ui.write_line("");

    for (key, value) in &report.system {
        ui.write_line(format!("{key}: {value}"));
    }

    ui.write_line("");
    ui.write_line("WinBrew Paths");
    render_runtime_section(ui, &report.runtime, "Paths");
    ui.write_line("");

    ui.write_line("WinBrew Settings");
    render_runtime_section(ui, &report.runtime, "Core");
}

/// Renders a named runtime report section, or a warning if it's missing.
///
/// A missing section here would mean the report-building code in
/// `winbrew-app` stopped emitting a section this command expects -- an
/// internal inconsistency between crates, not something a user did. That
/// shouldn't take down the whole `info` command (which the user may be
/// running specifically to diagnose a problem); it should be visible but
/// non-fatal.
fn render_runtime_section<W: Write>(ui: &mut Ui<W>, report: &RuntimeReport, title: &str) {
    match runtime_section(report, title) {
        Some(section) => ui.display_key_values(&section.entries),
        None => ui.warn(format!(
            "'{title}' section is missing from the runtime report (internal inconsistency)."
        )),
    }
}

fn runtime_section<'a>(report: &'a RuntimeReport, title: &str) -> Option<&'a ReportSection> {
    report
        .sections
        .iter()
        .find(|section| section.title == title)
}

#[cfg(test)]
mod tests {
    use super::{render_runtime_section, run_with_ui};
    use crate::app::AppContext;
    use crate::commands::test_support::{buffer_text, buffered_ui};
    use crate::database::Config;
    use tempfile::tempdir;
    use winbrew_app::models::domains::reporting::{ReportSection, RuntimeReport};
    use winbrew_ui::UiSettings;

    #[test]
    fn run_with_ui_renders_runtime_information() {
        let temp_dir = tempdir().expect("temp dir");
        let config = Config::load_at(temp_dir.path()).expect("config should load");
        let app = AppContext::from_config(&config).expect("app context should build");
        let (mut ui, out, err) = buffered_ui(UiSettings::default());

        run_with_ui(&mut ui, &app).expect("info should succeed");

        let out = buffer_text(&out);
        let err = buffer_text(&err);

        assert!(out.contains("WinBrew Package Manager v"));
        assert!(out.contains("Copyright (c) 2026 The WinBrew Contributors."));
        assert!(out.contains("Licensed under either of MIT or Apache 2.0 at your option."));
        assert!(out.contains("Windows:"));
        assert!(out.contains("System Architecture:"));
        assert!(out.contains("WinBrew Paths"));
        assert!(out.contains("WinBrew Settings"));
        assert!(out.contains("Key"));
        assert!(out.contains("Value"));
        assert!(err.is_empty());
    }

    /// A missing "Paths"/"Core" section here would mean winbrew-app's
    /// report-building code stopped emitting a section this command
    /// expects -- an internal inconsistency between crates. This used to
    /// panic via `.expect(...)`, taking down the whole `info` command
    /// (which a user may be running specifically to diagnose a problem)
    /// instead of surfacing the inconsistency and continuing.
    #[test]
    fn render_runtime_section_warns_instead_of_panicking_when_section_is_missing() {
        let (mut ui, _out, err) = buffered_ui(UiSettings::default());
        let report = RuntimeReport::new(vec![ReportSection {
            title: "Paths".to_string(),
            entries: vec![("root".to_string(), "C:\\winbrew".to_string())],
        }]);

        render_runtime_section(&mut ui, &report, "Core");

        let err = buffer_text(&err);
        assert!(err.contains("'Core' section is missing"));
    }
}
