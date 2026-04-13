//! Search command wrapper for catalog queries.
//!
//! The wrapper handles catalog-unavailable fallback text, empty-state output,
//! and the final result count for package searches.

use anyhow::Result;
use std::io::Write;

use crate::{CommandContext, app::search};
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext, query: &[String]) -> Result<()> {
    let mut ui = ctx.ui();
    run_with_ui(&mut ui, query)
}

fn run_with_ui<W: Write>(ui: &mut Ui<W>, query: &[String]) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::run_with_ui;
    use crate::commands::test_support::{buffer_text, buffered_ui};
    use crate::database::{self, Config};
    use tempfile::tempdir;
    use winbrew_ui::UiSettings;

    #[test]
    fn run_with_ui_reports_catalog_unavailable() {
        let temp_dir = tempdir().expect("temp dir");
        let config = Config::load_at(temp_dir.path()).expect("config should load");
        database::init(&config.resolved_paths()).expect("database should initialize");

        let (mut ui, out, err) = buffered_ui(UiSettings::default());
        let query = ["contoso".to_string()];

        run_with_ui(&mut ui, &query).expect("search should succeed");

        assert!(buffer_text(&out).trim().is_empty());
        assert!(
            buffer_text(&err)
                .contains("Package catalog not available. Run `winbrew update` first.")
        );
    }
}
