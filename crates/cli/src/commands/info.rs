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
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext) -> Result<()> {
    let mut ui = ctx.ui();
    run_with_ui(&mut ui, ctx.app())
}

fn run_with_ui<W: Write>(ui: &mut Ui<W>, app: &AppContext) -> Result<()> {
    ui.page_title("System Information");

    let report = info::collect(&app.sections, &app.paths)?;
    ui.notice(format!("Version: {}", report.version));

    for section in report.runtime.sections {
        ui.notice(&section.title);
        ui.display_key_values(&section.entries);
        ui.info("");
    }

    ui.success("Runtime settings displayed.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_with_ui;
    use crate::app::AppContext;
    use crate::commands::test_support::{buffer_text, buffered_ui};
    use crate::database::Config;
    use tempfile::tempdir;
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

        assert!(out.contains("Key"));
        assert!(out.contains("Value"));
        assert!(err.contains("Version: "));
        assert!(err.contains("Core"));
        assert!(err.contains("Paths"));
        assert!(err.contains("Runtime settings displayed."));
    }
}
