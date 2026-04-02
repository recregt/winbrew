use anyhow::Result;

use crate::{
    services::{info as info_service, version},
    ui::Ui,
};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("System Information");
    ui.notice(format!("Version: {}", version::version_string()));

    let report = info_service::runtime_report()?;

    for section in report.sections {
        ui.notice(&section.title);
        ui.display_key_values(&section.entries);
        ui.info("");
    }

    ui.success("Runtime settings displayed.");

    Ok(())
}
