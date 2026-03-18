use anyhow::Result;

use crate::{operations::list, ui::Ui};

pub fn run() -> Result<()> {
    let ui = Ui::new();
    ui.page_title("Winbrew Packages");

    let packages = list::list_packages()?;
    ui.display_packages(&packages);

    Ok(())
}
