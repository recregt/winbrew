use anyhow::Result;

use crate::{AppContext, services::search, ui::Ui};

pub fn run(ctx: &AppContext, query: &[String]) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Package Catalog");

    let query_text = query.join(" ");

    let packages = match search::search_packages(&query_text) {
        Ok(packages) => packages,
        Err(err) if search::is_catalog_unavailable(&err) => {
            ui.notice("Package catalog not available. Run `winbrew update` first.");
            return Ok(());
        }
        Err(err) => return Err(err),
    };

    if packages.is_empty() {
        ui.notice(format!("No catalog packages matching '{}'.", query_text));
        return Ok(());
    }

    ui.display_catalog_packages(&packages);
    ui.info(format!("\nTotal: {} package(s) found.", packages.len()));

    Ok(())
}
