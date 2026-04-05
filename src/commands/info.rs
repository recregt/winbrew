use anyhow::Result;

use crate::{
    AppContext,
    services::{info as info_service, version},
    ui::Ui,
};

pub fn run(ctx: &AppContext) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("System Information");
    ui.notice(format!("Version: {}", version::version_string()));

    let report = info_service::runtime_report(ctx)?;

    for section in report.sections {
        ui.notice(&section.title);
        ui.display_key_values(&section.entries);
        ui.info("");
    }

    ui.success("Runtime settings displayed.");

    Ok(())
}
