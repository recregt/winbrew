//! Search command wrapper for catalog queries.
//!
//! The wrapper handles catalog-unavailable fallback text, empty-state output,
//! and the final result count for package searches.

use anyhow::Result;

use crate::{CommandContext, app::search};

pub fn run(ctx: &CommandContext, query: &[String]) -> Result<()> {
    let mut ui = ctx.ui();
    ui.page_title("Package Catalog");

    let query_text = query.join(" ");

    let packages = match search::search_packages(&query_text) {
        Ok(packages) => packages,
        Err(search::SearchError::CatalogUnavailable) => {
            ui.notice("Package catalog not available. Run `winbrew update` first.");
            return Ok(());
        }
        Err(search::SearchError::Unexpected(err)) => return Err(err),
    };

    if packages.is_empty() {
        ui.notice(format!("No catalog packages matching '{query_text}'."));
        return Ok(());
    }

    ui.display_catalog_packages(&packages);
    ui.info(format!("\nTotal: {} package(s) found.", packages.len()));

    Ok(())
}
