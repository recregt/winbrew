use anyhow::Result;

use crate::{operations::remover, ui::Ui};

pub fn run(name: &str, yes: bool) -> Result<()> {
    let ui = Ui::new();
    ui.page_title("Remove");

    if !yes {
        let confirmed = ui.confirm("Do you want to remove this package and its shims?", false)?;

        if !confirmed {
            ui.notice("Aborted.");
            return Ok(());
        }
    }

    ui.spinner(format!("Removing {name}..."), || remover::remove(name))?;

    ui.success(format!("{name} was removed."));

    Ok(())
}
