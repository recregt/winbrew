use anyhow::Result;

use crate::{database, ui::Ui};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("System Information");
    ui.notice(format!("Version: {}", version_string()));

    let report = database::get_runtime_report()?;

    for section in report.sections {
        ui.notice(&section.title);
        ui.display_key_values(&section.entries);
        ui.info("");
    }

    ui.success("Runtime settings displayed.");

    Ok(())
}

fn version_string() -> String {
    format!(
        "{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("WINBREW_GIT_HASH")
    )
}
