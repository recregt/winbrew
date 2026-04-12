use anyhow::Result;

use crate::{CommandContext, app::update};

pub fn run(ctx: &CommandContext) -> Result<()> {
    let mut ui = ctx.ui();
    ui.page_title("Update Package Catalog");

    let progress = ui.progress_bar();

    let result = update::refresh_catalog(
        &ctx.app().paths,
        |total_bytes| {
            if let Some(total_bytes) = total_bytes {
                progress.set_length(total_bytes);
            }
            progress.set_message("Downloading catalog bundle");
        },
        |downloaded_bytes| {
            progress.inc(downloaded_bytes);
        },
    );

    progress.finish_and_clear();
    result?;

    ui.success("Package catalog updated.");
    Ok(())
}
