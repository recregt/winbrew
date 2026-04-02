use anyhow::Result;

use crate::{services::update, ui::Ui};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Update Package Catalog");

    update::refresh_catalog()?;

    ui.success("Package catalog updated.");
    Ok(())
}
