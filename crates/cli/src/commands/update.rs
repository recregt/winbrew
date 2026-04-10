use anyhow::Result;

use crate::{AppContext, Ui, app::update};

pub fn run(ctx: &AppContext) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Update Package Catalog");

    let progress = ui.progress_bar();

    let result = update::refresh_catalog(
        &ctx.paths,
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
