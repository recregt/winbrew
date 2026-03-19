use anyhow::Result;

use crate::{ops::list, ui::Ui};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Winbrew Packages");

    let packages = list::list_packages()?;
    ui.display_packages(&packages);

    Ok(())
}
