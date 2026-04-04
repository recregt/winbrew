use anyhow::Result;

use crate::{services::update, ui::Ui};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Update Package Catalog");

    let progress = ui.progress_bar();

    let result = update::refresh_catalog(
        |total_bytes| {
            if let Some(total_bytes) = total_bytes {
                progress.set_length(total_bytes);
            }
            progress.set_message("Downloading catalog.db");
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
