use anyhow::Result;

use crate::{services::list, ui::Ui};

pub fn run(query: &[String]) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Installed Packages");

    let query_text = (!query.is_empty()).then(|| query.join(" "));

    let packages = list::list_packages(query_text.as_deref())?;

    if packages.is_empty() {
        match query_text {
            Some(q) => ui.notice(format!("No installed packages matching '{}'.", q)),
            None => ui.notice("No packages are currently installed."),
        }

        return Ok(());
    }

    ui.display_packages(&packages);
    ui.info(format!("\nTotal: {} package(s) installed.", packages.len()));

    Ok(())
}
