use anyhow::Result;

use crate::{CommandContext, Ui, app::info};

pub fn run(ctx: &CommandContext) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("System Information");

    let report = info::collect(&ctx.sections, &ctx.paths)?;
    ui.notice(format!("Version: {}", report.version));

    for section in report.runtime.sections {
        ui.notice(&section.title);
        ui.display_key_values(&section.entries);
        ui.info("");
    }

    ui.success("Runtime settings displayed.");

    Ok(())
}
